use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

use crate::sources::Session;

fn line(s: &Session) -> String {
    let when = s
        .started
        .map(|d| d.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".into());
    format!(
        "{:<12} {:<6} {:<16} ${:>9.2}  {:>13} tok  {}",
        when,
        s.source,
        s.model.chars().take(16).collect::<String>(),
        s.cost_usd,
        s.total_tokens(),
        short_cwd(&s.cwd).chars().take(42).collect::<String>()
    )
}

fn short_cwd(cwd: &str) -> String {
    let p = std::path::Path::new(cwd);
    p.file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_string())
}

pub fn run(sessions: Vec<Session>) -> anyhow::Result<()> {
    let total: f64 = sessions.iter().map(|s| s.cost_usd).sum();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res: anyhow::Result<()> = loop {
        terminal.draw(|f| {
            let items: Vec<ListItem> = sessions
                .iter()
                .map(|s| ListItem::new(line(s)))
                .collect();
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " burnrate — {} sessions · ${:.2} total · q to quit ",
                        sessions.len(),
                        total
                    )),
            );
            f.render_widget(list, f.area());
        })?;
        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(k) = event::read()? {
                if k.code == KeyCode::Char('q') {
                    break Ok(());
                }
            }
        }
    };

    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    res
}
