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
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::error::Result;
use crate::session::{GroupTree, Status, Storage};
use crate::tmux::{PromptDetector, TmuxManager};

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

/// Tree item for switcher display
#[derive(Debug, Clone)]
enum SwitcherItem {
    Group { name: String, depth: usize },
    Session { idx: usize, depth: usize },
}

pub async fn run_switcher(profile: &str) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, groups) = storage.load().await?;

    let manager = Arc::new(TmuxManager::new());
    let mut analytics = crate::analytics::ActivityTracker::new(profile).await;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let _guard = TermGuard;
    terminal.clear()?;

    let mut query = String::new();
    let mut tree_items: Vec<SwitcherItem>;
    let mut flat_matches: Vec<usize>;
    let mut selected: usize = 0;
    let mut list_state = ListState::default();

    let mut tick_count: u64 = 0;
    let mut last_cache_refresh = Instant::now();

    // Status probing state
    let mut status_by_id: HashMap<String, Status> = HashMap::new();
    let mut last_tmux_activity: HashMap<String, i64> = HashMap::new();
    let mut last_tmux_activity_change: HashMap<String, Instant> = HashMap::new();
    let mut last_status_probe: HashMap<String, Instant> = HashMap::new();

    // Build tree view (group-organized)
    let build_tree = |groups: &GroupTree, instances: &[crate::session::Instance]| -> Vec<SwitcherItem> {
        use std::collections::BTreeMap;
        
        let mut by_group: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut ungrouped: Vec<usize> = Vec::new();
        
        for (i, inst) in instances.iter().enumerate() {
            if inst.group_path.is_empty() {
                ungrouped.push(i);
            } else {
                by_group.entry(inst.group_path.clone()).or_default().push(i);
            }
        }
        
        // Sort ungrouped by last_accessed_at desc
        ungrouped.sort_by(|&a, &b| {
            instances[b].last_accessed_at.cmp(&instances[a].last_accessed_at)
        });
        
        let mut items: Vec<SwitcherItem> = Vec::new();
        
        // Root groups first
        let mut roots: Vec<String> = groups
            .all_groups()
            .into_iter()
            .map(|g| g.path)
            .filter(|p| !p.contains('/'))
            .collect();
        roots.sort();
        
        fn visit(
            items: &mut Vec<SwitcherItem>,
            groups: &GroupTree,
            instances: &[crate::session::Instance],
            by_group: &BTreeMap<String, Vec<usize>>,
            path: &str,
            depth: usize,
        ) {
            let name = groups
                .get_group(path)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| path.split('/').last().unwrap_or(path).to_string());
            
            items.push(SwitcherItem::Group {
                name,
                depth,
            });
            
            // Child groups
            let mut children = groups.children(path);
            children.sort();
            for c in children {
                visit(items, groups, instances, by_group, &c, depth + 1);
            }
            
            // Sessions in this group
            if let Some(sessions) = by_group.get(path) {
                let mut sorted = sessions.clone();
                sorted.sort_by(|&a, &b| {
                    instances[b].last_accessed_at.cmp(&instances[a].last_accessed_at)
                });
                for idx in sorted {
                    items.push(SwitcherItem::Session { idx, depth: depth + 1 });
                }
            }
        }
        
        for r in roots {
            visit(&mut items, groups, instances, &by_group, &r, 0);
        }
        
        // Ungrouped sessions at bottom
        for idx in ungrouped {
            items.push(SwitcherItem::Session { idx, depth: 0 });
        }
        
        items
    };

    // Build flat matches (fuzzy search)
    let build_flat = |query: &str, instances: &[crate::session::Instance]| -> Vec<usize> {
        let q = query.trim();
        if q.is_empty() {
            let mut all: Vec<usize> = (0..instances.len()).collect();
            all.sort_by(|&a, &b| {
                instances[b]
                    .last_accessed_at
                    .cmp(&instances[a].last_accessed_at)
            });
            return all.into_iter().take(50).collect();
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
        scored.into_iter().map(|(_, idx)| idx).take(50).collect()
    };

    // Initial build
    tree_items = build_tree(&groups, &instances);
    flat_matches = build_flat(&query, &instances);
    list_state.select(Some(0));

    let tick_rate = Duration::from_millis(250);
    let result = loop {
        tick_count = tick_count.wrapping_add(1);

        // Keep tmux cache fresh.
        if last_cache_refresh.elapsed() >= Duration::from_secs(1) {
            let _ = manager.refresh_cache().await;
            last_cache_refresh = Instant::now();
        }

        // Probe statuses for visible sessions
        let now = Instant::now();
        let visible_sessions: Vec<usize> = if query.trim().is_empty() {
            // Tree mode - collect session indices from tree items
            tree_items.iter().filter_map(|item| {
                if let SwitcherItem::Session { idx, .. } = item { Some(*idx) } else { None }
            }).take(20).collect()
        } else {
            // Flat mode
            flat_matches.iter().copied().take(20).collect()
        };
        
        for idx in visible_sessions {
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

            // Track activity changes (but don't infer Running from it)
            let activity_changed = prev_activity.is_some_and(|a| activity > a);
            if activity_changed || prev_activity.is_none() {
                last_tmux_activity.insert(inst.id.clone(), activity);
                if activity_changed {
                    last_tmux_activity_change.insert(inst.id.clone(), now);
                }
            }

            // Decide whether to probe
            let settled = last_tmux_activity_change
                .get(id)
                .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(2));
            let need_probe = last_status_probe
                .get(id)
                .is_none_or(|t| now.duration_since(*t) >= Duration::from_secs(2));

            let should_probe =
                (settled && need_probe) || activity_changed || prev_activity.is_none();

            if !should_probe {
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

        // Determine display mode and item count
        let is_tree_mode = query.trim().is_empty();
        let item_count = if is_tree_mode { tree_items.len() } else { flat_matches.len() };
        
        // Clamp selection
        if selected >= item_count && item_count > 0 {
            selected = item_count - 1;
        }
        list_state.select(if item_count > 0 { Some(selected) } else { None });

        terminal.draw(|f| {
            draw_switcher(
                f,
                &instances,
                &query,
                &tree_items,
                &flat_matches,
                &mut list_state,
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
                        // Find selected session
                        let session_idx = if is_tree_mode {
                            tree_items.get(selected).and_then(|item| {
                                if let SwitcherItem::Session { idx, .. } = item { Some(*idx) } else { None }
                            })
                        } else {
                            flat_matches.get(selected).copied()
                        };
                        
                        if let Some(idx) = session_idx {
                            let inst = &instances[idx];
                            let tmux_name = inst.tmux_name();
                            
                            // Record analytics: switcher usage
                            let _ = analytics.record_switch(&inst.id, &inst.title).await;
                            
                            let _ = manager
                                .set_environment_global("AGENTHAND_LAST_SESSION", &tmux_name)
                                .await;
                            manager.switch_client(&tmux_name).await?;
                        }
                        break Ok(());
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        if query.trim().is_empty() {
                            tree_items = build_tree(&groups, &instances);
                        }
                        flat_matches = build_flat(&query, &instances);
                        selected = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if item_count > 0 {
                            if selected == 0 {
                                selected = item_count - 1;
                            } else {
                                selected -= 1;
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if item_count > 0 {
                            selected = (selected + 1) % item_count;
                        }
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            query.push(ch);
                            flat_matches = build_flat(&query, &instances);
                            selected = 0;
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

fn draw_switcher(
    f: &mut Frame,
    instances: &[crate::session::Instance],
    query: &str,
    tree_items: &[SwitcherItem],
    flat_matches: &[usize],
    list_state: &mut ListState,
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

    let is_tree_mode = query.trim().is_empty();
    let selected = list_state.selected().unwrap_or(0);

    let mut items: Vec<ListItem> = Vec::new();
    
    if is_tree_mode {
        // Tree view mode
        for (row, item) in tree_items.iter().enumerate() {
            match item {
                SwitcherItem::Group { name, depth } => {
                    let indent = "  ".repeat(*depth);
                    let style = if row == selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    };
                    let line = Line::from(vec![
                        Span::raw(indent),
                        Span::styled("▸ ", style),
                        Span::styled(name.clone(), style),
                    ]);
                    items.push(ListItem::new(line));
                }
                SwitcherItem::Session { idx, depth } => {
                    let inst = &instances[*idx];
                    let indent = "  ".repeat(*depth);
                    
                    let status = status_by_id.get(&inst.id).copied().unwrap_or(Status::Idle);
                    let (icon, color) = match status {
                        Status::Waiting => (waiting_anim(tick), Color::Blue),
                        Status::Running => (running_anim(tick), Color::Yellow),
                        Status::Idle => ("○", Color::DarkGray),
                        Status::Error => ("✕", Color::Red),
                        Status::Starting => ("⋯", Color::Cyan),
                    };
                    
                    let is_selected = row == selected;
                    let icon_style = if is_selected {
                        Style::default().fg(color).bg(Color::Cyan)
                    } else {
                        Style::default().fg(color)
                    };
                    let text_style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let path_style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    
                    let line = Line::from(vec![
                        Span::raw(indent),
                        Span::styled(icon, icon_style),
                        Span::raw(" "),
                        Span::styled(inst.title.clone(), text_style),
                        Span::raw("  "),
                        Span::styled(
                            inst.project_path.to_string_lossy().to_string(),
                            path_style,
                        ),
                    ]);
                    items.push(ListItem::new(line));
                }
            }
        }
    } else {
        // Flat fuzzy search mode
        for (row, &idx) in flat_matches.iter().enumerate() {
            let inst = &instances[idx];

            let rank_style = if row == 0 {
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
            ]);

            items.push(ListItem::new(line));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Span::styled(
            "(no sessions)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let title_str = if is_tree_mode {
        "Sessions (type to search)".to_string()
    } else {
        format!("Search: {query}")
    };
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title_str))
        .highlight_symbol("");
    f.render_stateful_widget(list, list_area, list_state);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Type", Style::default().fg(Color::Cyan)),
        Span::raw(": filter  "),
        Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
        Span::raw(": select  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::raw(": switch  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(": close"),
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
