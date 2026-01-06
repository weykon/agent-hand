use std::io;
use std::sync::Arc;

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::error::Result;
use crate::session::Storage;
use crate::tmux::TmuxManager;

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

pub async fn run_switcher(profile: &str) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, _) = storage.load().await?;

    let manager = Arc::new(TmuxManager::new());

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let _guard = TermGuard;
    terminal.clear()?;

    let mut query = String::new();
    let mut matches: Vec<usize> = Vec::new();
    let mut selected: usize = 0;

    let update_matches = |query: &str, matches: &mut Vec<usize>, selected: &mut usize| {
        let q = query.trim();

        // Default view: show sessions immediately (most-recent first)
        if q.is_empty() {
            let mut all: Vec<usize> = (0..instances.len()).collect();
            all.sort_by(|&a, &b| {
                instances[b]
                    .last_accessed_at
                    .cmp(&instances[a].last_accessed_at)
                    .then(instances[a].title.cmp(&instances[b].title))
            });
            matches.clear();
            matches.extend(all.into_iter().take(50));
            *selected = 0;
            return;
        }

        let mut scored: Vec<(i32, usize)> = Vec::new();
        for (idx, inst) in instances.iter().enumerate() {
            let hay = format!(
                "{} {} {} {}",
                inst.title,
                inst.group_path,
                inst.project_path.to_string_lossy(),
                inst.id
            );
            if let Some(score) = fuzzy_score(q, &hay) {
                scored.push((score, idx));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        matches.clear();
        matches.extend(scored.into_iter().map(|(_, idx)| idx).take(50));
        if *selected >= matches.len() {
            *selected = 0;
        }
    };

    update_matches(&query, &mut matches, &mut selected);

    let tick_rate = std::time::Duration::from_millis(250);
    let result = loop {
        terminal.draw(|f| draw(f, &instances, &query, &matches, selected))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                CrosstermEvent::Key(key) => match key.code {
                    KeyCode::Esc => break Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break Ok(())
                    }
                    KeyCode::Enter => {
                        if let Some(&idx) = matches.get(selected) {
                            let tmux_name = instances[idx].tmux_name();
                            // Record last active session for future dashboard UX/features.
                            let _ = manager
                                .set_environment_global("AGENTHAND_LAST_SESSION", &tmux_name)
                                .await;
                            manager.switch_client(&tmux_name).await?;
                        }
                        break Ok(());
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        update_matches(&query, &mut matches, &mut selected);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !matches.is_empty() {
                            if selected == 0 {
                                selected = matches.len() - 1;
                            } else {
                                selected -= 1;
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !matches.is_empty() {
                            selected = (selected + 1) % matches.len();
                        }
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            query.push(ch);
                            update_matches(&query, &mut matches, &mut selected);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    };

    result
}

fn draw(
    f: &mut Frame,
    instances: &[crate::session::Instance],
    query: &str,
    matches: &[usize],
    selected: usize,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("Switch Session")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let list_area = chunks[1];
    f.render_widget(Clear, list_area);

    let q = query.trim();

    let mut items: Vec<ListItem> = Vec::new();
    for (row, &idx) in matches.iter().enumerate() {
        let inst = &instances[idx];

        // Ranking highlight when user is typing:
        // - best match: green
        // - other matches: yellow
        let rank_style = if q.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else if row == 0 {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let style = if row == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            rank_style
        };

        let group = if inst.group_path.is_empty() {
            "(none)"
        } else {
            inst.group_path.as_str()
        };

        let line = Line::from(vec![
            Span::styled(inst.title.clone(), style),
            Span::raw("  "),
            Span::styled(format!("[{group}]"), Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled(
                inst.project_path.to_string_lossy().to_string(),
                if row == selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::raw("  "),
            Span::styled(
                format!("({})", inst.id),
                if row == selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]);

        items.push(ListItem::new(line));
    }

    if matches.is_empty() {
        items.push(ListItem::new(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Search: {query}")),
    );
    f.render_widget(list, list_area);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Type", Style::default().fg(Color::Cyan)),
        Span::raw(": filter  "),
        Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
        Span::raw(": select  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::raw(": switch  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(": close  "),
        Span::styled("tmux", Style::default().fg(Color::DarkGray)),
        Span::raw(": agentdeck_rs_<id>"),
    ]))
    .wrap(Wrap { trim: true })
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let q = query.to_lowercase();
    let t = text.to_lowercase();

    let mut score: i32 = 0;
    let mut last_match: Option<usize> = None;
    let mut pos = 0usize;

    for ch in q.chars() {
        if let Some(found) = t[pos..].find(ch) {
            let idx = pos + found;
            score += 10;
            if let Some(prev) = last_match {
                if idx == prev + 1 {
                    score += 15;
                } else {
                    score -= (idx.saturating_sub(prev) as i32).min(10);
                }
            } else {
                score -= idx.min(15) as i32;
            }
            last_match = Some(idx);
            pos = idx + ch.len_utf8();
        } else {
            return None;
        }
    }

    Some(score)
}
