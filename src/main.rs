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

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Print a cost report as a table (default action)
    Report,
    /// Launch the TUI
    Tui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let days = cli.days.max(1) as i64;
    let now = chrono::Utc::now();
    let since = now - chrono::Duration::days(days);

    let sessions = sources::scan(since)?;
    let sessions = report::filter_recent(sessions, since);

    match &cli.cmd {
        Some(Cmd::Tui) => tui::run(sessions),
        _ => report::print_table(sessions, days),
    }
}
