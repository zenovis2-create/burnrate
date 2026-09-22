use crate::sources::{ScanDiagnostics, Session};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
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
    unpriced_tokens: u64,
    unpriced_models: BTreeSet<String>,
    redundant_reads: u64,
}

impl ReportSummary {
    fn add(&mut self, session: &Session) {
        self.session_count += 1;
        self.total_cost_usd += session.cost_usd;
        self.total_tokens += session.total_tokens();
        self.unpriced_sessions += if session.priced { 0 } else { 1 };
        self.unpriced_tokens += session.unpriced_tokens;
        self.unpriced_models
            .extend(session.unpriced_models.iter().cloned());
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
    window_basis: &'static str,
    cost_basis: &'static str,
    data_source: &'static str,
    billing_status: &'static str,
    quota_status: &'static str,
    scan: Option<&'a ScanDiagnostics>,
    summary: ReportSummary,
    sources: Vec<SourceSummary>,
    sessions: Vec<&'a Session>,
}

fn build_json_report<'a>(
    sessions: &'a [Session],
    days: i64,
    generated_at: DateTime<Utc>,
    diagnostics: Option<&'a ScanDiagnostics>,
) -> JsonReport<'a> {
    let mut summary = ReportSummary::default();
    let mut by_source: BTreeMap<&'static str, ReportSummary> = BTreeMap::new();
    for session in sessions {
        summary.add(session);
        by_source.entry(session.source).or_default().add(session);
    }

    JsonReport {
        generated_at,
        window_days: days,
        window_basis: "full_sessions_with_recent_filesystem_activity",
        cost_basis: "standard_api_rate_known_subtotal",
        data_source: if diagnostics.is_some() {
            "local_logs"
        } else {
            "synthetic"
        },
        billing_status: "not_observed",
        quota_status: "not_observed",
        scan: diagnostics,
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
    diagnostics: Option<&ScanDiagnostics>,
) -> anyhow::Result<()> {
    let report = build_json_report(sessions, days, generated_at, diagnostics);
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &report)?;
    writeln!(writer)?;
    Ok(())
}

pub fn print_table(
    sessions: Vec<Session>,
    days: i64,
    generated_at: DateTime<Utc>,
    diagnostics: Option<&ScanDiagnostics>,
) -> anyhow::Result<()> {
    let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.total_tokens()).sum();
    let unpriced = sessions.iter().filter(|s| !s.priced).count();

    println!(
        "burnrate — active in last {} days — {} sessions, ${:.2} known subtotal (API-rate est.), {} tokens",
        days,
        sessions.len(),
        total_cost,
        human_tok(total_tokens)
    );
    println!(
        "Snapshot: {} — full-session totals, not period-only spend",
        generated_at.to_rfc3339()
    );
    println!("Estimates only; actual bill and provider quota not observed.");
    if let Some(diagnostics) = diagnostics {
        println!("{}", diagnostics.summary());
    } else {
        println!("Synthetic demo — no local logs scanned.");
    }
    if unpriced > 0 {
        let tokens: u64 = sessions.iter().map(|s| s.unpriced_tokens).sum();
        let models: BTreeSet<&str> = sessions
            .iter()
            .flat_map(|s| s.unpriced_models.iter().map(String::as_str))
            .collect();
        println!("Pricing incomplete: {unpriced} sessions / {tokens} tokens unpriced (+? means unknown, not free).");
        println!(
            "Unpriced models: {}",
            models.into_iter().collect::<Vec<_>>().join(", ")
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
            println!(
                "  {:<8} ${:>8.2} known subtotal   {:>10} tok",
                src,
                cost,
                human_tok(tok)
            );
        }
    }

    let mut waste: Vec<&Session> = sessions.iter().filter(|s| s.reread_extras > 0).collect();
    waste.sort_by_key(|s| std::cmp::Reverse(s.reread_extras));
    if !waste.is_empty() {
        println!("\nrepeat-read signals (same path; not proof of waste):");
        for s in waste.iter().take(5) {
            println!(
                "  {} {:<6} {:<16} {} repeated reads  top: {} x{}  ({})",
                fmt_date(s.activity_at()),
                s.source,
                s.model.chars().take(16).collect::<String>(),
                s.reread_extras,
                short_cwd(&s.top_reread_file)
                    .chars()
                    .take(30)
                    .collect::<String>(),
                s.top_reread_count,
                s.cost_label()
            );
        }
    }

    let ranked = ranked_sessions(&sessions);
    println!();
    println!(
        "{:<12} {:<7} {:<18} {:>10} {:>11} {:>7} {:>8}  CWD",
        "ACTIVE", "SRC", "MODEL", "EST. COST", "TOKENS", "CACHE%", "RE-READS"
    );
    for s in ranked.iter().take(15) {
        println!(
            "{:<12} {:<7} {:<18} {:>10} {:>11} {:>6.0}% {:>8}  {}",
            fmt_date(s.activity_at()),
            s.source,
            s.model.chars().take(18).collect::<String>(),
            s.cost_label(),
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

        let value = serde_json::to_value(build_json_report(&sessions, 7, now, None)).unwrap();

        assert_eq!(value["window_days"], 7);
        assert_eq!(value["summary"]["session_count"], 5);
        assert_eq!(value["summary"]["redundant_reads"], 63);
        assert_eq!(value["sessions"][0]["cwd"], "/work/checkout");
        assert_eq!(value["sources"][0]["source"], "claude");
        assert_eq!(value["sources"][1]["source"], "codex");
        assert_eq!(value["data_source"], "synthetic");
        assert!(value["scan"].is_null());
    }

    #[test]
    fn json_report_discloses_unknown_prices_and_scan_errors() {
        let now = Utc::now();
        let mut sessions = crate::sources::demo_sessions(now);
        sessions[0].priced = false;
        sessions[0].unpriced_models = vec!["unknown-model".into()];
        sessions[0].unpriced_tokens = 120;
        let diagnostics = ScanDiagnostics {
            malformed_lines: 2,
            io_errors: 1,
            ..Default::default()
        };
        let value =
            serde_json::to_value(build_json_report(&sessions, 7, now, Some(&diagnostics))).unwrap();
        assert_eq!(value["cost_basis"], "standard_api_rate_known_subtotal");
        assert_eq!(
            value["window_basis"],
            "full_sessions_with_recent_filesystem_activity"
        );
        assert_eq!(value["billing_status"], "not_observed");
        assert_eq!(value["quota_status"], "not_observed");
        assert_eq!(value["summary"]["unpriced_sessions"], 1);
        assert_eq!(value["summary"]["unpriced_tokens"], 120);
        assert_eq!(value["summary"]["unpriced_models"][0], "unknown-model");
        assert_eq!(value["scan"]["malformed_lines"], 2);
        assert_eq!(value["scan"]["io_errors"], 1);
    }
}
