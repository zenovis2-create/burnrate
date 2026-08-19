use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};

use crate::sources::Session;

fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn human_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn run(sessions: Vec<Session>) -> anyhow::Result<()> {
    let total_cost: f64 = sessions.iter().map(|s| s.cost_usd).sum();
    let redundant_reads: u64 = sessions.iter().map(|s| s.reread_extras).sum();
    let total_input: u64 = sessions
        .iter()
        .map(|s| s.input_tokens + s.cache_read_tokens)
        .sum();
    let cached_input: u64 = sessions.iter().map(|s| s.cache_read_tokens).sum();
    let cache_pct = if total_input == 0 {
        0.0
    } else {
        cached_input as f64 / total_input as f64 * 100.0
    };
    let top_offender = sessions
        .iter()
        .filter(|s| s.top_reread_count > 1)
        .max_by_key(|s| s.top_reread_count)
        .map(|s| format!("{} x{}", short_name(&s.top_reread_file), s.top_reread_count))
        .unwrap_or_else(|| "none detected".into());

    let mut ranked: Vec<&Session> = sessions.iter().collect();
    ranked.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res: anyhow::Result<()> = loop {
        terminal.draw(|frame| {
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let summary = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("  ${total_cost:.2}"),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  spent", Style::default().fg(Color::Gray)),
                    Span::raw("     "),
                    Span::styled(
                        format!("{redundant_reads}"),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  redundant reads", Style::default().fg(Color::Gray)),
                    Span::raw("     "),
                    Span::styled(
                        format!("{cache_pct:.0}%"),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  cached input", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  top offender  ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        top_offender.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(Span::styled(
                        " burnrate — htop for coding-agent spend ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )),
            );
            frame.render_widget(summary, areas[0]);

            let header = Row::new([
                "WHEN", "SRC", "MODEL", "COST", "TOKENS", "CACHE", "RE-READS", "PROJECT",
            ])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);
            let rows = ranked.iter().map(|s| {
                let when = s
                    .started
                    .map(|d| d.format("%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".into());
                let cost_style = if s.cost_usd >= 10.0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let reread_style = if s.reread_extras > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Row::new(vec![
                    Cell::from(when),
                    Cell::from(s.source),
                    Cell::from(s.model.chars().take(18).collect::<String>()),
                    Cell::from(format!("${:.2}", s.cost_usd)).style(cost_style),
                    Cell::from(human_tokens(s.total_tokens())),
                    Cell::from(format!("{:.0}%", s.cache_share() * 100.0)),
                    Cell::from(s.reread_extras.to_string()).style(reread_style),
                    Cell::from(short_name(&s.cwd)),
                ])
            });
            let table = Table::new(
                rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(7),
                    Constraint::Length(19),
                    Constraint::Length(9),
                    Constraint::Length(9),
                    Constraint::Length(7),
                    Constraint::Length(10),
                    Constraint::Min(12),
                ],
            )
            .header(header)
            .column_spacing(1)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(format!(" {} sessions · ranked by cost ", sessions.len())),
            );
            frame.render_widget(table, areas[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("local-only", Style::default().fg(Color::Green)),
                Span::styled(
                    "  ·  your logs never leave this machine  ·  q quit",
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
            frame.render_widget(footer, areas[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break Ok(());
                }
            }
        }
    };

    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    res
}
