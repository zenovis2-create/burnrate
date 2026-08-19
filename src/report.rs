use crate::sources::Session;
use chrono::{DateTime, Utc};

pub fn filter_recent(sessions: Vec<Session>, since: DateTime<Utc>) -> Vec<Session> {
    sessions
        .into_iter()
        .filter(|s| s.started.map(|t| t >= since).unwrap_or(true))
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

    // waste: redundant file re-reads
    let mut waste: Vec<&Session> = sessions.iter().filter(|s| s.reread_extras > 0).collect();
    waste.sort_by_key(|s| std::cmp::Reverse(s.reread_extras));
    if !waste.is_empty() {
        println!("\ntop waste (agent re-reading the same file):");
        for s in waste.iter().take(5) {
            println!(
                "  {} {:<6} {:<16} {} redundant re-reads  worst: {} x{}  (${:.2})",
                fmt_date(s.started),
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

    let mut ranked: Vec<&Session> = sessions.iter().collect();
    ranked.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!();
    println!(
        "{:<12} {:<7} {:<18} {:>10} {:>11} {:>7} {:>8}  CWD",
        "WHEN", "SRC", "MODEL", "COST", "TOKENS", "CACHE%", "RE-READS"
    );
    for s in ranked.iter().take(15) {
        println!(
            "{:<12} {:<7} {:<18} {:>10.2} {:>11} {:>6.0}% {:>8}  {}",
            fmt_date(s.started),
            s.source,
            s.model.chars().take(18).collect::<String>(),
            s.cost_usd,
            s.total_tokens(),
            s.cache_share() * 100.0,
            s.reread_extras,
            short_cwd(&s.cwd).chars().take(40).collect::<String>()
        );
    }
    Ok(())
}
