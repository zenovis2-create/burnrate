use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::pricing;

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub source: &'static str, // "claude" | "codex"
    pub started: Option<DateTime<Utc>>,
    /// Last filesystem activity observed for the session log.
    pub updated: Option<DateTime<Utc>>,
    pub cwd: String,
    pub model: String,
    pub input_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub priced: bool,
    pub reread_extras: u64,
    pub top_reread_file: String,
    pub top_reread_count: u64,
}

impl Session {
    pub fn activity_at(&self) -> Option<DateTime<Utc>> {
        self.updated.as_ref().or(self.started.as_ref()).copied()
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens().saturating_add(self.output_tokens)
    }

    pub fn total_input_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_write_tokens)
            .saturating_add(self.cache_read_tokens)
    }

    /// Share of input context served from the provider cache (0..1).
    pub fn cache_share(&self) -> f64 {
        let inp = self.total_input_tokens();
        if inp == 0 {
            0.0
        } else {
            self.cache_read_tokens as f64 / inp as f64
        }
    }
}

/// Deterministic, synthetic sessions used to record public demos without
/// exposing local project names or log contents.
pub fn demo_sessions(now: DateTime<Utc>) -> Vec<Session> {
    let mut sessions = vec![
        Session {
            source: "claude",
            started: Some(now - chrono::Duration::hours(2)),
            updated: Some(now - chrono::Duration::hours(2)),
            cwd: "/work/checkout".into(),
            model: "claude-opus-4-1".into(),
            input_tokens: 1_420_000,
            cache_write_tokens: 340_000,
            cache_read_tokens: 2_260_000,
            output_tokens: 238_000,
            cost_usd: 0.0,
            priced: false,
            reread_extras: 31,
            top_reread_file: "/work/checkout/package-lock.json".into(),
            top_reread_count: 12,
        },
        Session {
            source: "codex",
            started: Some(now - chrono::Duration::hours(8)),
            updated: Some(now - chrono::Duration::hours(8)),
            cwd: "/work/api".into(),
            model: "gpt-5-codex".into(),
            input_tokens: 2_870_000,
            cache_write_tokens: 0,
            cache_read_tokens: 4_040_000,
            output_tokens: 694_000,
            cost_usd: 0.0,
            priced: false,
            reread_extras: 0,
            top_reread_file: String::new(),
            top_reread_count: 0,
        },
        Session {
            source: "claude",
            started: Some(now - chrono::Duration::days(1)),
            updated: Some(now - chrono::Duration::days(1)),
            cwd: "/work/dashboard".into(),
            model: "claude-sonnet-4".into(),
            input_tokens: 780_000,
            cache_write_tokens: 210_000,
            cache_read_tokens: 1_310_000,
            output_tokens: 176_000,
            cost_usd: 0.0,
            priced: false,
            reread_extras: 19,
            top_reread_file: "/work/dashboard/schema.graphql".into(),
            top_reread_count: 9,
        },
        Session {
            source: "claude",
            started: Some(now - chrono::Duration::days(2)),
            updated: Some(now - chrono::Duration::days(2)),
            cwd: "/work/mobile".into(),
            model: "claude-sonnet-4".into(),
            input_tokens: 510_000,
            cache_write_tokens: 130_000,
            cache_read_tokens: 670_000,
            output_tokens: 121_000,
            cost_usd: 0.0,
            priced: false,
            reread_extras: 13,
            top_reread_file: "/work/mobile/pnpm-lock.yaml".into(),
            top_reread_count: 7,
        },
        Session {
            source: "codex",
            started: Some(now - chrono::Duration::days(3)),
            updated: Some(now - chrono::Duration::days(3)),
            cwd: "/work/docs".into(),
            model: "gpt-5-codex".into(),
            input_tokens: 390_000,
            cache_write_tokens: 0,
            cache_read_tokens: 610_000,
            output_tokens: 83_000,
            cost_usd: 0.0,
            priced: false,
            reread_extras: 0,
            top_reread_file: String::new(),
            top_reread_count: 0,
        },
    ];

    for session in &mut sessions {
        (session.cost_usd, session.priced) = pricing::cost(
            &session.model,
            session.input_tokens,
            session.cache_write_tokens,
            session.cache_read_tokens,
            session.output_tokens,
        );
    }
    sessions
}

/// Scan Claude Code + Codex logs touched since `since`.
pub fn scan(since: DateTime<Utc>) -> Result<Vec<Session>> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let claude = home.join(".claude").join("projects");
        if claude.exists() {
            for f in jsonl_files_touched_since(&claude, since) {
                if let Some(mut s) = parse_claude(&f.path) {
                    s.updated = Some(f.updated);
                    out.push(s);
                }
            }
        }
        let codex = home.join(".codex").join("sessions");
        if codex.exists() {
            for f in jsonl_files_touched_since(&codex, since) {
                if let Some(mut s) = parse_codex(&f.path) {
                    s.updated = Some(f.updated);
                    out.push(s);
                }
            }
        }
    }
    Ok(out)
}

struct LogFile {
    path: PathBuf,
    updated: DateTime<Utc>,
}

fn jsonl_files_touched_since(root: &Path, since: DateTime<Utc>) -> Vec<LogFile> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            let file_type = match e.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                stack.push(p);
            } else if file_type.is_file()
                && p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("jsonl"))
                    .unwrap_or(false)
            {
                let updated = match e
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(DateTime::<Utc>::from)
                {
                    Ok(updated) if updated >= since => updated,
                    _ => continue,
                };
                files.push(LogFile { path: p, updated });
            }
        }
    }
    files
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

#[derive(Default)]
struct ModelUsage {
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
}

impl ModelUsage {
    fn add(&mut self, input: u64, cache_write: u64, cache_read: u64, output: u64) {
        self.input = self.input.saturating_add(input);
        self.cache_write = self.cache_write.saturating_add(cache_write);
        self.cache_read = self.cache_read.saturating_add(cache_read);
        self.output = self.output.saturating_add(output);
    }

    fn total_tokens(&self) -> u64 {
        self.input
            .saturating_add(self.cache_write)
            .saturating_add(self.cache_read)
            .saturating_add(self.output)
    }
}

/// Claude Code: ~/.claude/projects/<proj>/<session>.jsonl
/// Assistant lines carry message.model + message.usage.
fn parse_claude(path: &Path) -> Option<Session> {
    let mut cwd: Option<String> = None;
    let mut started: Option<DateTime<Utc>> = None;
    let mut input: u64 = 0;
    let mut cache_write: u64 = 0;
    let mut cache_read: u64 = 0;
    let mut output: u64 = 0;
    let mut model_usage: BTreeMap<String, ModelUsage> = BTreeMap::new();
    let mut read_files: HashMap<String, u64> = HashMap::new();

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
            input = input.saturating_add(i);
            cache_write = cache_write.saturating_add(cw);
            cache_read = cache_read.saturating_add(cr);
            output = output.saturating_add(o);
            model_usage.entry(m).or_default().add(i, cw, cr, o);
            if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                for item in content {
                    if item.get("type").and_then(|x| x.as_str()) == Some("tool_use")
                        && item.get("name").and_then(|x| x.as_str()) == Some("Read")
                    {
                        if let Some(p) = item
                            .get("input")
                            .and_then(|i| i.get("file_path"))
                            .and_then(|x| x.as_str())
                        {
                            let count = read_files.entry(p.to_string()).or_insert(0);
                            *count = count.saturating_add(1);
                        }
                    }
                }
            }
        }
    }

    // Claude Code can switch models inside one session. Price each model's
    // usage independently and display the model responsible for most tokens.
    let model = model_usage
        .iter()
        .max_by_key(|(_, usage)| usage.total_tokens())
        .map(|(model, _)| model.clone())?;
    let mut cost_usd = 0.0;
    let mut priced = true;
    for (model, usage) in &model_usage {
        let (model_cost, model_priced) = pricing::cost(
            model,
            usage.input,
            usage.cache_write,
            usage.cache_read,
            usage.output,
        );
        cost_usd += model_cost;
        priced &= model_priced;
    }
    let mut reread_extras: u64 = 0;
    let mut top_reread_file = String::new();
    let mut top_reread_count: u64 = 0;
    for (p, c) in &read_files {
        if *c > 1 {
            reread_extras = reread_extras.saturating_add(c - 1);
        }
        if *c > top_reread_count {
            top_reread_count = *c;
            top_reread_file = p.clone();
        }
    }
    Some(Session {
        source: "claude",
        started,
        updated: None,
        cwd: cwd.unwrap_or_default(),
        model,
        input_tokens: input,
        cache_write_tokens: cache_write,
        cache_read_tokens: cache_read,
        output_tokens: output,
        cost_usd,
        priced,
        reread_extras,
        top_reread_file,
        top_reread_count,
    })
}

/// Codex: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl
/// Cumulative token_count events; last one in the file is the session total.
fn parse_codex(path: &Path) -> Option<Session> {
    let mut cwd: Option<String> = None;
    let mut started: Option<DateTime<Utc>> = None;
    let mut model = String::new();
    // Codex input_tokens includes cached and cache-write tokens. Normalize the
    // final tuple into mutually exclusive buckets before pricing it.
    let mut total: Option<(u64, u64, u64, u64)> = None;

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
                            let cw = t
                                .get("cache_write_input_tokens")
                                .and_then(|x| x.as_u64())
                                .unwrap_or(0);
                            let o = t.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                            total = Some((i, c, cw, o));
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

    let (input_total, cached, cache_write, output) = total?;
    let input = input_total
        .saturating_sub(cached)
        .saturating_sub(cache_write);
    let (cost_usd, priced) = pricing::cost(&model, input, cache_write, cached, output);
    Some(Session {
        source: "codex",
        started,
        updated: None,
        cwd: cwd.unwrap_or_default(),
        model: if model.is_empty() {
            String::from("codex")
        } else {
            model
        },
        input_tokens: input,
        cache_write_tokens: cache_write,
        cache_read_tokens: cached,
        output_tokens: output,
        cost_usd,
        priced,
        reread_extras: 0,
        top_reread_file: String::new(),
        top_reread_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_jsonl(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "burnrate-{name}-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parses_claude_usage_and_repeated_reads() {
        let line1 = r#"{"type":"assistant","cwd":"/work/app","timestamp":"2026-08-19T00:00:00Z","message":{"model":"claude-sonnet-4","usage":{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":200,"output_tokens":20},"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/work/app/package-lock.json"}}]}}"#;
        let line2 = r#"{"type":"assistant","message":{"model":"claude-sonnet-4","usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":20,"output_tokens":5},"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/work/app/package-lock.json"}}]}}"#;
        let path = temp_jsonl("claude", &format!("{line1}\n{line2}\n"));

        let session = parse_claude(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(session.cwd, "/work/app");
        assert_eq!(session.input_tokens, 110);
        assert_eq!(session.cache_read_tokens, 220);
        assert_eq!(session.reread_extras, 1);
        assert_eq!(session.top_reread_count, 2);
        assert!(session.priced);
    }

    #[test]
    fn parses_latest_codex_cumulative_total() {
        let contents = concat!(
            r#"{"timestamp":"2026-08-19T00:00:00Z","payload":{"cwd":"/work/api","model":"gpt-5-codex","type":"session_meta"}}"#,
            "\n",
            r#"{"timestamp":"2026-08-19T00:01:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100}}}}"#,
            "\n",
            r#"{"timestamp":"2026-08-19T00:02:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1800,"cached_input_tokens":700,"cache_write_input_tokens":300,"output_tokens":220}}}}"#,
            "\n"
        );
        let path = temp_jsonl("codex", contents);

        let session = parse_codex(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(session.cwd, "/work/api");
        assert_eq!(session.model, "gpt-5-codex");
        assert_eq!(session.input_tokens, 800);
        assert_eq!(session.cache_read_tokens, 700);
        assert_eq!(session.cache_write_tokens, 300);
        assert_eq!(session.output_tokens, 220);
        assert!(session.priced);
    }

    #[test]
    fn prices_each_model_in_a_mixed_claude_session() {
        let opus = r#"{"type":"assistant","message":{"model":"claude-opus-4-1","usage":{"input_tokens":1000000,"output_tokens":0}}}"#;
        let sonnet = r#"{"type":"assistant","message":{"model":"claude-sonnet-4","usage":{"input_tokens":0,"output_tokens":2000000}}}"#;
        let path = temp_jsonl("claude-mixed-model", &format!("{opus}\n{sonnet}\n"));

        let session = parse_claude(&path).unwrap();
        std::fs::remove_file(path).unwrap();

        assert_eq!(session.model, "claude-sonnet-4");
        assert!((session.cost_usd - 45.0).abs() < f64::EPSILON);
        assert!(session.priced);
    }

    #[test]
    fn skips_claude_logs_without_assistant_usage() {
        let path = temp_jsonl(
            "claude-no-usage",
            r#"{"type":"user","cwd":"/work/app","message":{"content":"hello"}}"#,
        );

        let session = parse_claude(&path);
        std::fs::remove_file(path).unwrap();

        assert!(session.is_none());
    }

    #[test]
    fn demo_costs_are_derived_from_the_price_table() {
        let now = Utc::now();
        let sessions = demo_sessions(now);
        let total: f64 = sessions.iter().map(|s| s.cost_usd).sum();

        assert_eq!(sessions.len(), 5);
        for session in &sessions {
            let (expected, priced) = pricing::cost(
                &session.model,
                session.input_tokens,
                session.cache_write_tokens,
                session.cache_read_tokens,
                session.output_tokens,
            );
            assert!((session.cost_usd - expected).abs() < f64::EPSILON);
            assert_eq!(session.priced, priced);
        }
        assert!((total - 71.53525).abs() < 0.000_001);
        assert_eq!(sessions.iter().map(|s| s.reread_extras).sum::<u64>(), 63);
    }

    #[test]
    fn cache_share_counts_cache_writes_as_uncached_input() {
        let session = &demo_sessions(Utc::now())[0];
        let expected = 2_260_000.0 / (1_420_000.0 + 340_000.0 + 2_260_000.0);

        assert!((session.cache_share() - expected).abs() < f64::EPSILON);
    }
}
