use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::pricing;

#[derive(Debug, Clone)]
pub struct Session {
    pub source: &'static str, // "claude" | "codex"
    pub id: String,
    pub started: Option<DateTime<Utc>>,
    pub cwd: String,
    pub model: String,
    pub input_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub priced: bool,
}

impl Session {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.cache_write_tokens + self.cache_read_tokens + self.output_tokens
    }
}

/// Scan Claude Code + Codex logs touched since `since`.
pub fn scan(since: DateTime<Utc>) -> Result<Vec<Session>> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude").join("projects");
        if claude.exists() {
            for f in jsonl_files_touched_since(&claude, since) {
                if let Some(s) = parse_claude(&f) {
                    out.push(s);
                }
            }
        }
        let codex = home.join(".codex").join("sessions");
        if codex.exists() {
            for f in jsonl_files_touched_since(&codex, since) {
                if let Some(s) = parse_codex(&f) {
                    out.push(s);
                }
            }
        }
    }
    Ok(out)
}

fn jsonl_files_touched_since(root: &Path, since: DateTime<Utc>) -> Vec<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            let mt = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| Some(DateTime::<Utc>::from(t)));
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "jsonl").unwrap_or(false)
                && mt.map(|t| t >= since).unwrap_or(false)
            {
                files.push(p);
            }
        }
    }
    files
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

/// Claude Code: ~/.claude/projects/<proj>/<session>.jsonl
/// Assistant lines carry message.model + message.usage.
fn parse_claude(path: &Path) -> Option<Session> {
    let mut id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut started: Option<DateTime<Utc>> = None;
    let mut input: u64 = 0;
    let mut cache_write: u64 = 0;
    let mut cache_read: u64 = 0;
    let mut output: u64 = 0;
    let mut model = String::from("claude");
    let mut model_tokens: u64 = 0;

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line.ok() {
            Some(l) => l,
            None => continue,
        };
        let val: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let v = match val.as_object() {
            Some(o) => o,
            None => continue,
        };
        // borrow-checker friendly: clone small strings only when present
        if let Some(sid) = v.get("sessionId").and_then(|x| x.as_str()) {
            if id.is_none() {
                id = Some(sid.to_string());
            }
        }
        if let Some(c) = v.get("cwd").and_then(|x| x.as_str()) {
            if cwd.is_none() {
                cwd = Some(c.to_string());
            }
        }
        if let Some(ts) = v.get("timestamp").and_then(|x| x.as_str()) {
            if started.is_none() {
                started = parse_ts(ts);
            }
        }
        if v.get("type").and_then(|x| x.as_str()) == Some("assistant") {
            let msg = match v.get("message") {
                Some(m) => m,
                None => continue,
            };
            let m = match msg.get("model").and_then(|x| x.as_str()) {
                Some(m) => m.to_string(),
                None => continue,
            };
            let u = match msg.get("usage") {
                Some(u) => u,
                None => continue,
            };
            let i = u.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            let cw = u
                .get("cache_creation_input_tokens")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let cr = u
                .get("cache_read_input_tokens")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let o = u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            input += i;
            cache_write += cw;
            cache_read += cr;
            output += o;
            if model_tokens == 0 {
                model = m;
            }
            model_tokens += i + cw + cr + o;
        }
    }

    let (cost_usd, priced) = pricing::cost(&model, input, cache_write, cache_read, output);
    Some(Session {
        source: "claude",
        id: id.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        }),
        started,
        cwd: cwd.unwrap_or_default(),
        model,
        input_tokens: input,
        cache_write_tokens: cache_write,
        cache_read_tokens: cache_read,
        output_tokens: output,
        cost_usd,
        priced,
    })
}

/// Codex: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
/// Cumulative token_count events; last one in the file is the session total.
fn parse_codex(path: &Path) -> Option<Session> {
    let mut id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut started: Option<DateTime<Utc>> = None;
    let mut model = String::new();
    let mut total: Option<(u64, u64, u64)> = None; // (input_incl_cached, cached, output)

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line.ok() {
            Some(l) => l,
            None => continue,
        };
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            let obj = match v.as_object() {
                Some(o) => o,
                None => continue,
            };
            if let Some(ts) = obj.get("timestamp").and_then(|x| x.as_str()) {
                if started.is_none() {
                    started = parse_ts(ts);
                }
            }
            if let Some(payload) = obj.get("payload") {
                if let Some(sid) = payload.get("session_id").and_then(|x| x.as_str()) {
                    if id.is_none() {
                        id = Some(sid.to_string());
                    }
                }
                if let Some(c) = payload.get("cwd").and_then(|x| x.as_str()) {
                    if cwd.is_none() {
                        cwd = Some(c.to_string());
                    }
                }
                if model.is_empty() {
                    if let Some(m) = payload.get("model").and_then(|x| x.as_str()) {
                        model = m.to_string();
                    }
                }
                if payload.get("type").and_then(|x| x.as_str()) == Some("token_count") {
                    if let Some(info) = payload.get("info") {
                        if let Some(t) = info.get("total_token_usage") {
                            let i = t.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                            let c = t
                                .get("cached_input_tokens")
                                .and_then(|x| x.as_u64())
                                .unwrap_or(0);
                            let o = t.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                            total = Some((i, c, o));
                        }
                    }
                }
            }
            if model.is_empty() {
                if let Some(m) = obj.get("model").and_then(|x| x.as_str()) {
                    model = m.to_string();
                }
            }
        } else if model.is_empty() && line.contains("\"model\":\"") {
            // cheap fallback for lines that fail full parse (e.g. truncation)
            if let Some(pos) = line.find("\"model\":\"") {
                let rest = &line[pos + 9..];
                if let Some(end) = rest.find('"') {
                    model = rest[..end].to_string();
                }
            }
        }
    }

    let (input_total, cached, output) = total?;
    let input = input_total.saturating_sub(cached);
    let (cost_usd, priced) = pricing::cost(&model, input, 0, cached, output);
    Some(Session {
        source: "codex",
        id: id.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        }),
        started,
        cwd: cwd.unwrap_or_default(),
        model: if model.is_empty() {
            String::from("codex")
        } else {
            model
        },
        input_tokens: input,
        cache_write_tokens: 0,
        cache_read_tokens: cached,
        output_tokens: output,
        cost_usd,
        priced,
    })
}
