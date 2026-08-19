use crate::sources::Session;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;

pub fn filter_recent(sessions: Vec<Session>, since: DateTime<Utc>) -> Vec<Session> {
    sessions
        .into_iter()
        .filter(|s| s.activity_at().map(|t| t >= since).unwrap_or(true))
        .collect()
}

fn short_cwd(cwd: &str) -> String {
    let p = std::path::Path::new(cwd);
    p.file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_string())
}

fn fmt_date(t: Option<DateTime<Utc>>) -> String {
    t.map(|d| d.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".into())
}

fn human_tok(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn ranked_sessions(sessions: &[Session]) -> Vec<&Session> {
    let mut ranked: Vec<&Session> = sessions.iter().collect();
    ranked.sort_by(|a, b| {
        b.cost_usd
            .total_cmp(&a.cost_usd)
            .then_with(|| b.total_tokens().cmp(&a.total_tokens()))
            .then_with(|| b.activity_at().cmp(&a.activity_at()))
            .then_with(|| a.cwd.cmp(&b.cwd))
    });
    ranked
}

#[derive(Debug, Default, Serialize)]
struct ReportSummary {
    session_count: usize,
    total_cost_usd: f64,
    total_tokens: u64,
    unpriced_sessions: usize,
    redundant_reads: u64,
}

impl ReportSummary {
    fn add(&mut self, session: &Session) {
        self.session_count += 1;
        self.total_cost_usd += session.cost_usd;
        self.total_tokens += session.total_tokens();
        self.unpriced_sessions += if session.priced { 0 } else { 1 };
        self.redundant_reads += session.reread_extras;
    }
}

#[derive(Debug, Serialize)]
struct SourceSummary {
    source: &'static str,
    #[serde(flatten)]
    totals: ReportSummary,
}

#[derive(Debug, Serialize)]
struct JsonReport<'a> {
    generated_at: DateTime<Utc>,
    window_days: i64,
    summary: ReportSummary,
    sources: Vec<SourceSummary>,
    sessions: Vec<&'a Session>,
}

fn build_json_report(
    sessions: &[Session],
    days: i64,
    generated_at: DateTime<Utc>,
) -> JsonReport<'_> {
    let mut summary = ReportSummary::default();
    let mut by_source: BTreeMap<&'static str, ReportSummary> = BTreeMap::new();
    for session in sessions {
        summary.add(session);
        by_source.entry(session.source).or_default().add(session);
    }

    JsonReport {
        generated_at,
        window_days: days,
        summary,
        sources: by_source
            .into_iter()
            .map(|(source, totals)| SourceSummary { source, totals })
            .collect(),
        sessions: ranked_sessions(sessions),
    }
}

pub fn print_json(
    sessions: &[Session],
    days: i64,
    generated_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let report = build_json_report(sessions, days, generated_at);
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &report)?;
    writeln!(writer)?;
    Ok(())
}

pub fn print_table(sessions: Vec<Session>, days: i64) -> anyhow::Result<()> {
    let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();
    let unpriced = sessions.iter().filter(|s| !s.priced).count();

    println!(
        "burnrate — last {} days — {} sessions, ${:.2} (API-rate est.), {} tokens",
        days,
        sessions.len(),
        total_cost,
        human_tok(total_tokens)
    );
    if unpriced > 0 {
        println!(
            "( {} sessions had unpriced models — counted as $0 )",
            unpriced
        );
    }

    let by_source: Vec<(&'static str, f64, u64)> = {
        let mut acc: Vec<(&'static str, f64, u64)> = vec![("claude", 0.0, 0), ("codex", 0.0, 0)];
        for s in &sessions {
            let e = acc.iter_mut().find(|x| x.0 == s.source);
            if let Some(e) = e {
                e.1 += s.cost_usd;
                e.2 += s.total_tokens();
            }
        }
        acc
    };
    for (src, cost, tok) in by_source {
        if tok > 0 || cost > 0.0 {
            println!("  {:<8} ${:>8.2}   {:>10} tok", src, cost, human_tok(tok));
        }
    }

    let mut waste: Vec<&Session> = sessions.iter().filter(|s| s.reread_extras > 0).collect();
    waste.sort_by_key(|s| std::cmp::Reverse(s.reread_extras));
    if !waste.is_empty() {
        println!("\ntop waste (agent re-reading the same file):");
        for s in waste.iter().take(5) {
            println!(
                "  {} {:<6} {:<16} {} redundant re-reads  worst: {} x{}  (${:.2})",
                fmt_date(s.activity_at()),
                s.source,
                s.model.chars().take(16).collect::<String>(),
                s.reread_extras,
                short_cwd(&s.top_reread_file)
                    .chars()
                    .take(30)
                    .collect::<String>(),
                s.top_reread_count,
                s.cost_usd
            );
        }
    }

    let ranked = ranked_sessions(&sessions);
    println!();
    println!(
        "{:<12} {:<7} {:<18} {:>10} {:>11} {:>7} {:>8}  CWD",
        "ACTIVE", "SRC", "MODEL", "COST", "TOKENS", "CACHE%", "RE-READS"
    );
    for s in ranked.iter().take(15) {
        println!(
            "{:<12} {:<7} {:<18} {:>10.2} {:>11} {:>6.0}% {:>8}  {}",
            fmt_date(s.activity_at()),
            s.source,
            s.model.chars().take(18).collect::<String>(),
            s.cost_usd,
            human_tok(s.total_tokens()),
            s.cache_share() * 100.0,
            s.reread_extras,
            short_cwd(&s.cwd).chars().take(40).collect::<String>()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_filter_uses_last_activity_instead_of_start_time() {
        let now = Utc::now();
        let since = now - chrono::Duration::days(7);
        let mut sessions = crate::sources::demo_sessions(now);
        sessions[0].started = Some(now - chrono::Duration::days(30));
        sessions[0].updated = Some(now - chrono::Duration::hours(1));
        sessions[1].started = Some(now - chrono::Duration::days(30));
        sessions[1].updated = Some(now - chrono::Duration::days(8));

        let filtered = filter_recent(sessions, since);

        assert_eq!(filtered.len(), 4);
        assert!(filtered.iter().any(|s| s.cwd == "/work/checkout"));
        assert!(!filtered.iter().any(|s| s.cwd == "/work/api"));
    }

    #[test]
    fn json_report_has_deterministic_totals_and_cost_ranking() {
        let now = Utc::now();
        let sessions = crate::sources::demo_sessions(now);

        let value = serde_json::to_value(build_json_report(&sessions, 7, now)).unwrap();

        assert_eq!(value["window_days"], 7);
        assert_eq!(value["summary"]["session_count"], 5);
        assert_eq!(value["summary"]["redundant_reads"], 63);
        assert_eq!(value["sessions"][0]["cwd"], "/work/checkout");
        assert_eq!(value["sources"][0]["source"], "claude");
        assert_eq!(value["sources"][1]["source"], "codex");
    }
}
