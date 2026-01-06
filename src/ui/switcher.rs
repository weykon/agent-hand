use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
use crate::session::{Status, Storage};
use crate::tmux::{PromptDetector, TmuxManager};

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

    let mut tick_count: u64 = 0;
    let mut last_cache_refresh = Instant::now();

    // Status probing state (same idea as dashboard: cheap activity gating + occasional capture-pane).
    let mut status_by_id: HashMap<String, Status> = HashMap::new();
    let mut last_tmux_activity: HashMap<String, i64> = HashMap::new();
    let mut last_tmux_activity_change: HashMap<String, Instant> = HashMap::new();
    let mut last_status_probe: HashMap<String, Instant> = HashMap::new();

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

    let tick_rate = Duration::from_millis(250);
    let result = loop {
        tick_count = tick_count.wrapping_add(1);

        // Keep tmux cache fresh.
        if last_cache_refresh.elapsed() >= Duration::from_secs(1) {
            let _ = manager.refresh_cache().await;
            last_cache_refresh = Instant::now();
        }

        // Probe statuses for visible rows (bounded so switcher stays snappy).
        let now = Instant::now();
        for &idx in matches.iter().take(20) {
            let inst = &instances[idx];
            let id = inst.id.as_str();
            let tmux_session = inst.tmux_name();

            if !manager.session_exists(&tmux_session).unwrap_or(false) {
                status_by_id.insert(inst.id.clone(), Status::Idle);
                last_tmux_activity.remove(id);
                last_tmux_activity_change.remove(id);
                last_status_probe.remove(id);
                continue;
            }

            let activity = manager.session_activity(&tmux_session).unwrap_or(0);
            let prev_activity = last_tmux_activity.get(id).copied();
            if prev_activity.is_none() || prev_activity.is_some_and(|a| activity > a) {
                last_tmux_activity.insert(inst.id.clone(), activity);
                last_tmux_activity_change.insert(inst.id.clone(), now);
                status_by_id.insert(inst.id.clone(), Status::Running);
                continue;
            }

            let settled = last_tmux_activity_change
                .get(id)
                .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(2));
            let need_probe = last_status_probe
                .get(id)
                .is_none_or(|t| now.duration_since(*t) >= Duration::from_secs(2));

            if !(settled && need_probe) {
                continue;
            }

            let content = manager
                .capture_pane(&tmux_session, 15)
                .await
                .unwrap_or_default();
            let detector = PromptDetector::new(inst.tool);
            let new_status = if detector.has_prompt(&content) {
                Status::Waiting
            } else if detector.is_busy(&content) {
                Status::Running
            } else {
                Status::Idle
            };

            status_by_id.insert(inst.id.clone(), new_status);
            last_status_probe.insert(inst.id.clone(), now);
        }

        terminal.draw(|f| {
            draw(
                f,
                &instances,
                &query,
                &matches,
                selected,
                &status_by_id,
                tick_count,
            )
        })?;

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

fn running_anim(tick: u64) -> &'static str {
    const FRAMES: [&str; 4] = ["·", "●", "⬤", "●"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn waiting_anim(tick: u64) -> &'static str {
    const FRAMES: [&str; 5] = ["!", "!", "!", "!", " "];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn draw(
    f: &mut Frame,
    instances: &[crate::session::Instance],
    query: &str,
    matches: &[usize],
    selected: usize,
    status_by_id: &HashMap<String, Status>,
    tick: u64,
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

        let status = status_by_id.get(&inst.id).copied().unwrap_or(Status::Idle);
        let (icon, color) = match status {
            Status::Waiting => (waiting_anim(tick), Color::Blue),
            Status::Running => (running_anim(tick), Color::Yellow),
            Status::Idle => ("○", Color::DarkGray),
            Status::Error => ("✕", Color::Red),
            Status::Starting => ("⋯", Color::Cyan),
        };
        let icon_style = if row == selected {
            Style::default().fg(color).bg(Color::Cyan)
        } else {
            Style::default().fg(color)
        };

        let line = Line::from(vec![
            Span::styled(icon, icon_style),
            Span::raw(" "),
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
