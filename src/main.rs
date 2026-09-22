mod pricing;
mod report;
mod sources;
mod tui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "burnrate", version, about = "htop for coding-agent spend")]
struct Cli {
    /// Only include sessions active in the last N days (default 7)
    #[arg(long, default_value_t = 7)]
    days: u32,

    /// Use synthetic sessions (handy for screenshots and trying the UI)
    #[arg(long)]
    demo: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Print a cost report as a table (default action)
    Report {
        /// Emit machine-readable JSON instead of the table
        #[arg(long)]
        json: bool,
    },
    /// Launch the TUI
    Tui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let days = cli.days.max(1) as i64;
    let now = chrono::Utc::now();
    let since = now - chrono::Duration::days(days);

    let (sessions, diagnostics) = if cli.demo {
        (sources::demo_sessions(now), None)
    } else {
        let scan = sources::scan(since)?;
        (
            report::filter_recent(scan.sessions, since),
            Some(scan.diagnostics),
        )
    };
    let snapshot = chrono::Utc::now();

    match cli.cmd {
        Some(Cmd::Tui) => tui::run(sessions, days, snapshot, diagnostics.as_ref()),
        Some(Cmd::Report { json: true }) => {
            report::print_json(&sessions, days, snapshot, diagnostics.as_ref())
        }
        Some(Cmd::Report { json: false }) | None => {
            report::print_table(sessions, days, snapshot, diagnostics.as_ref())
        }
    }
}
