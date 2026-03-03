use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::session::Status;
use crate::ui::TextInput;

use super::app::App;
use super::TreeItem;

fn running_anim(tick: u64) -> &'static str {
    // Claude-style small/medium/large dot pulse.
    const FRAMES: [&str; 4] = ["·", "●", "⬤", "●"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn waiting_anim(tick: u64) -> &'static str {
    // Blink to draw attention: ~1s on, ~0.3s off (tick is 250ms).
    const FRAMES: [&str; 5] = ["!", "!", "!", "!", " "];
    FRAMES[(tick as usize) % FRAMES.len()]
}

/// Render a TextInput with cursor visible when active
fn render_text_input(input: &TextInput, active: bool, base_style: Style) -> Vec<Span<'static>> {
    let text = input.text();
    let cursor_pos = input.cursor();

    if !active {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    // Split text at cursor position
    let (before, after) = text.split_at(cursor_pos);

    // Get the character at cursor (or space if at end)
    let (cursor_char, rest) = if after.is_empty() {
        (" ", "")
    } else {
        let mut chars = after.char_indices();
        let _ = chars.next(); // skip first char
        let rest_start = chars.next().map(|(i, _)| i).unwrap_or(after.len());
        let cursor_str = &after[..rest_start];
        (cursor_str, &after[rest_start..])
    };

    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);

    vec![
        Span::styled(before.to_string(), base_style),
        Span::styled(cursor_char.to_string(), cursor_style),
        Span::styled(rest.to_string(), base_style),
    ]
}

/// Main render function
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Render title
    render_title(f, chunks[0]);

    // Always render main content (dashboard stays visible behind modal)
    #[cfg(feature = "pro")]
    {
        if app.state() == crate::ui::AppState::ViewerMode {
            render_viewer_mode(f, chunks[1], app);
        } else if app.state() == crate::ui::AppState::Relationships {
            render_relationships(f, chunks[1], app);
        } else {
            render_main(f, chunks[1], app);
        }
    }
    #[cfg(not(feature = "pro"))]
    render_main(f, chunks[1], app);

    // Render status bar
    render_status_bar(f, chunks[2], app);

    // Help modal overlays on top when visible
    if app.help_visible() {
        render_help_modal(f, f.area());
    }

    if app.state() == crate::ui::AppState::Dialog {
        render_dialog(f, f.area(), app);
    }

    if app.state() == crate::ui::AppState::Search {
        render_search_popup(f, f.area(), app);
    }
}

/// Render title bar
fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("🦀 Agent Deck (Rust) Agent Hand")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, area);
}

fn render_main(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_session_list(f, cols[0], app);
    render_preview(f, cols[1], app);
}

/// Render session list (splits off active panel at top when premium + active sessions exist)
fn render_session_list(f: &mut Frame, area: Rect, app: &App) {
    #[cfg(feature = "pro")]
    {
        let is_pro = app.auth_token().map_or(false, |t| t.is_pro());
        let active = app.active_sessions();

        if is_pro && !active.is_empty() {
            // 2 border rows + 1 row per session, capped at 8 total rows
            let panel_h = (active.len() as u16 + 2).min(8);
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(panel_h), Constraint::Min(0)])
                .split(area);
            render_active_panel(f, rows[0], app, &active);
            render_session_tree(f, rows[1], app);
        } else {
            render_session_tree(f, area, app);
        }
    }
    #[cfg(not(feature = "pro"))]
    render_session_tree(f, area, app);
}

/// Render the active sessions panel (premium feature pinned above the session tree)
#[cfg(feature = "pro")]
fn render_active_panel(f: &mut Frame, area: Rect, app: &App, active: &[&crate::session::Instance]) {
    let focused = app.active_panel_focused();
    let selected = app.active_panel_selected();

    let items: Vec<ListItem> = active
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = focused && i == selected;
            let base = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };

            let status_icon = match s.status {
                Status::Waiting => waiting_anim(app.tick_count()),
                Status::Running => running_anim(app.tick_count()),
                Status::Error => "✕",
                Status::Starting => "⋯",
                Status::Idle => "○",
            };
            let status_color = match s.status {
                Status::Waiting => Color::Blue,
                Status::Running => Color::Yellow,
                Status::Error => Color::Red,
                Status::Starting => Color::Cyan,
                Status::Idle => Color::DarkGray,
            };

            let line = Line::from(vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(s.title.clone(), base.add_modifier(Modifier::BOLD)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!("⚡ Active ({})", active.len());
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );

    let mut state = if focused {
        ListState::default().with_selected(Some(selected))
    } else {
        ListState::default()
    };
    f.render_stateful_widget(list, area, &mut state);
}

/// Render the full session tree (groups + sessions)
fn render_session_tree(f: &mut Frame, area: Rect, app: &App) {
    let tree = app.tree();

    if tree.is_empty() {
        let empty = Paragraph::new("No sessions found.\n\nUse: agent-hand add ...\nPress 'n' to create.\nPress '?' for help.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Sessions"));

        f.render_widget(empty, area);
        return;
    }

    let tree_focused = {
        #[cfg(feature = "pro")]
        { !app.active_panel_focused() }
        #[cfg(not(feature = "pro"))]
        { true }
    };

    let items: Vec<ListItem> = tree
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = tree_focused && i == app.selected_index();
            let base = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };

            match item {
                TreeItem::Group { path, name, depth } => {
                    let indent = "  ".repeat(*depth);
                    let icon = if app.group_has_children(path) {
                        if app.is_group_expanded(path) {
                            "▾"
                        } else {
                            "▸"
                        }
                    } else {
                        " "
                    };

                    let line = Line::from(vec![
                        Span::styled(indent, Style::default()),
                        Span::styled(icon, Style::default().fg(Color::Magenta)),
                        Span::raw(" "),
                        Span::styled(name, base.add_modifier(Modifier::BOLD)),
                        Span::raw(" "),
                        Span::styled(format!("({})", path), Style::default().fg(Color::DarkGray)),
                    ]);
                    ListItem::new(line)
                }
                TreeItem::Session { id, depth } => {
                    let indent = "  ".repeat(*depth);
                    let s = app.session_by_id(id);

                    let (status_icon, status_color, title, label, label_color) =
                        if let Some(session) = s {
                            let status_icon = match session.status {
                                Status::Waiting => waiting_anim(app.tick_count()),
                                Status::Running => running_anim(app.tick_count()),
                                Status::Idle => {
                                    if app.is_attention_active(&session.id) {
                                        "✓"
                                    } else {
                                        "○"
                                    }
                                }
                                Status::Error => "✕",
                                Status::Starting => "⋯",
                            };

                            let status_color = match session.status {
                                Status::Waiting => Color::Blue,
                                Status::Running => Color::Yellow,
                                Status::Idle => {
                                    if app.is_attention_active(&session.id) {
                                        Color::Cyan
                                    } else {
                                        Color::DarkGray
                                    }
                                }
                                Status::Error => Color::Red,
                                Status::Starting => Color::Cyan,
                            };

                            (
                                status_icon,
                                status_color,
                                session.title.as_str(),
                                session.label.as_str(),
                                session.label_color,
                            )
                        } else {
                            (
                                "?",
                                Color::Red,
                                "<missing>",
                                "",
                                crate::session::LabelColor::Gray,
                            )
                        };

                    let label_color = match label_color {
                        crate::session::LabelColor::Gray => Color::DarkGray,
                        crate::session::LabelColor::Magenta => Color::Magenta,
                        crate::session::LabelColor::Cyan => Color::Cyan,
                        crate::session::LabelColor::Green => Color::Green,
                        crate::session::LabelColor::Yellow => Color::Yellow,
                        crate::session::LabelColor::Red => Color::Red,
                        crate::session::LabelColor::Blue => Color::Blue,
                    };

                    let mut spans = vec![
                        Span::styled(indent, Style::default()),
                        Span::styled(status_icon, Style::default().fg(status_color)),
                        Span::raw(" "),
                        Span::styled(title, base.add_modifier(Modifier::BOLD)),
                    ];

                    let label = label.trim();
                    if !label.is_empty() {
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled(
                            format!("[{label}]"),
                            Style::default().fg(label_color),
                        ));
                    }

                    // PTY leak warning badge
                    if let Some(session) = s {
                        if session.ptmx_count > 0 {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(
                                format!("⚠ {} pty", session.ptmx_count),
                                Style::default().fg(Color::Yellow),
                            ));
                        }

                        // Sharing badge (Premium)
                        if let Some(ref sharing) = session.sharing {
                            spans.push(Span::raw("  "));
                            if sharing.active && sharing.should_auto_expire() {
                                spans.push(Span::styled(
                                    format!("[share: {} expiring]", sharing.default_permission),
                                    Style::default().fg(Color::Yellow),
                                ));
                            } else if sharing.active {
                                spans.push(Span::styled(
                                    format!("[share: {}]", sharing.default_permission),
                                    Style::default().fg(Color::Green),
                                ));
                            } else {
                                spans.push(Span::styled(
                                    "[share: stopped]",
                                    Style::default().fg(Color::DarkGray),
                                ));
                            }
                        }

                        // Relationship indicator
                        {
                            let rel_count = app.relationships().iter().filter(|r| {
                                r.session_a_id == session.id || r.session_b_id == session.id
                            }).count();
                            if rel_count > 0 {
                                spans.push(Span::raw("  "));
                                spans.push(Span::styled(
                                    format!("[{}rel]", rel_count),
                                    Style::default().fg(Color::Blue),
                                ));
                            }
                        }
                    }

                    let line = Line::from(spans);
                    ListItem::new(line)
                }
            }
        })
        .collect();

    let border_style = if tree_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!(
                    "Tree ({}/{})",
                    app.selected_index() + 1,
                    tree.len()
                ),
                if tree_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ))
            .border_style(border_style),
    );

    #[cfg(feature = "pro")]
    {
        let mut state = if tree_focused {
            app.list_state().clone()
        } else {
            ListState::default()
        };
        f.render_stateful_widget(list, area, &mut state);
    }
    #[cfg(not(feature = "pro"))]
    {
        let mut state = ListState::default().with_selected(Some(app.selected_index()));
        f.render_stateful_widget(list, area, &mut state);
    }
}

fn render_preview(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.selected_item() {
        Some(TreeItem::Session { id, .. }) => app
            .session_by_id(id)
            .map(|s| format!("Preview • {}", s.title))
            .unwrap_or_else(|| "Preview".to_string()),
        Some(TreeItem::Group { name, .. }) => format!("Preview • {}", name),
        _ => "Preview".to_string(),
    };

    let p = Paragraph::new(app.preview())
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(p, area);
}

fn render_dialog(f: &mut Frame, area: Rect, app: &App) {
    if app.quit_confirm_dialog() {
        render_quit_confirm_dialog(f, area);
        return;
    }

    if let Some(d) = app.new_session_dialog() {
        render_new_session_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.delete_confirm_dialog() {
        render_delete_confirm_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.delete_group_dialog() {
        render_delete_group_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.fork_dialog() {
        render_fork_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.create_group_dialog() {
        render_create_group_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.move_group_dialog() {
        render_move_group_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.rename_session_dialog() {
        render_rename_session_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.tag_picker_dialog() {
        render_tag_picker_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.rename_group_dialog() {
        render_rename_group_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.settings_dialog() {
        render_settings_dialog(f, area, d);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.join_session_dialog() {
        render_join_session_dialog(f, area, d);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.share_dialog() {
        render_share_dialog(f, area, d);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.create_relationship_dialog() {
        render_create_relationship_dialog(f, area, d);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.annotate_dialog() {
        render_annotate_dialog(f, area, d);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.new_from_context_dialog() {
        render_new_from_context_dialog(f, area, d);
    }
}

fn render_new_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::NewSessionDialog) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_path_active = d.field == crate::ui::NewSessionField::Path;
    let is_title_active = d.field == crate::ui::NewSessionField::Title;
    let is_group_active = d.field == crate::ui::NewSessionField::Group;

    let mut path_spans = vec![Span::raw("Path:   ")];
    path_spans.extend(render_text_input(&d.path, is_path_active, base_style));

    let mut title_spans = vec![Span::raw("Title:  ")];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw("Group:  ")];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let mut lines = vec![
        Line::from(Span::styled(
            "New Session",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(path_spans),
    ];

    if d.path_will_be_created() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(
                "(not found; will create directory)",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    if d.path_suggestions_visible && !d.path_suggestions.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("Suggestions:", Style::default().fg(Color::DarkGray)),
        ]));
        let max_show = 8usize;
        let len = d.path_suggestions.len();
        let idx = d.path_suggestions_idx.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, s) in d
            .path_suggestions
            .iter()
            .enumerate()
            .skip(start)
            .take(max_show)
        {
            let style = if i == d.path_suggestions_idx {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("          "),
                Span::styled(s.clone(), style),
            ]));
        }
    }

    lines.extend([Line::from(title_spans), Line::from(group_spans)]);

    if d.field == crate::ui::NewSessionField::Group {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Groups (↑/↓ to select):",
            Style::default().fg(Color::DarkGray),
        )));

        if d.group_matches.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no matches)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let max_show = 8usize;
            let len = d.group_matches.len();
            let idx = d.group_selected.min(len.saturating_sub(1));
            let start = if len <= max_show {
                0
            } else if idx + 1 >= max_show {
                (idx + 1 - max_show).min(len - max_show)
            } else {
                0
            };

            for (i, g) in d
                .group_matches
                .iter()
                .enumerate()
                .skip(start)
                .take(max_show)
            {
                let label = if g.is_empty() { "(none)" } else { g.as_str() };
                let style = if i == d.group_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(label.to_string(), style),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab: complete path • ↑↓: pick • Enter: next/submit • Esc/Ctrl+C: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("New"));

    f.render_widget(p, popup_area);
}

fn render_fork_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ForkDialog) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::ForkField::Title;
    let is_group_active = d.field == crate::ui::ForkField::Group;

    let mut title_spans = vec![Span::raw("Title: ")];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw("Group: ")];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let lines = vec![
        Line::from(Span::styled(
            "Fork Session",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(group_spans),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: switch field • Enter: next/submit • Esc/Ctrl+C: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Fork"));

    f.render_widget(p, popup_area);
}

fn render_create_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::CreateGroupDialog) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw("Name:   ")];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            "Create Group",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            "Existing (↑/↓ to select):",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_show = 10usize;
        let len = d.matches.len();
        let idx = d.selected.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, g) in d.matches.iter().enumerate().skip(start).take(max_show) {
            let style = if i == d.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(g.to_string(), style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Type to filter/name • Enter: create • Esc/Ctrl+C: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Group"));

    f.render_widget(p, popup_area);
}

fn render_move_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::MoveGroupDialog) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw("Filter: ")];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            "Move Session to Group",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Title:  "),
            Span::styled(
                d.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            "Groups (↑/↓ to select):",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_show = 10usize;
        let len = d.matches.len();
        let idx = d.selected.min(len.saturating_sub(1));
        let start = if len <= max_show {
            0
        } else if idx + 1 >= max_show {
            (idx + 1 - max_show).min(len - max_show)
        } else {
            0
        };

        for (i, g) in d.matches.iter().enumerate().skip(start).take(max_show) {
            let label = if g.is_empty() { "(none)" } else { g.as_str() };
            let style = if i == d.selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(label.to_string(), style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Type to filter • Enter: apply • Esc/Ctrl+C: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Group"));

    f.render_widget(p, popup_area);
}

fn render_tag_picker_dialog(f: &mut Frame, area: Rect, d: &crate::ui::TagPickerDialog) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(popup_area);

    if d.tags.is_empty() {
        let empty = Paragraph::new(
            "(no tags found)\n\nTip: edit a session label first (r), then reuse it here.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Tag"));
        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = d
            .tags
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let fg = match t.color {
                    crate::session::LabelColor::Gray => Color::DarkGray,
                    crate::session::LabelColor::Magenta => Color::Magenta,
                    crate::session::LabelColor::Cyan => Color::Cyan,
                    crate::session::LabelColor::Green => Color::Green,
                    crate::session::LabelColor::Yellow => Color::Yellow,
                    crate::session::LabelColor::Red => Color::Red,
                    crate::session::LabelColor::Blue => Color::Blue,
                };

                let style = if i == d.selected {
                    Style::default()
                        .fg(fg)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg)
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("[{}]", t.name), style),
                ]))
            })
            .collect();

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Tag"));
        let mut state = ListState::default().with_selected(Some(d.selected));
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    let hint = Paragraph::new("↑/↓: select • Enter: apply • Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(hint, chunks[1]);
}

fn render_rename_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameSessionDialog) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::SessionEditField::Title;
    let is_label_active = d.field == crate::ui::SessionEditField::Label;

    let mut title_spans = vec![Span::raw("Title:  ")];
    title_spans.extend(render_text_input(&d.new_title, is_title_active, base_style));

    let mut label_spans = vec![Span::raw("Label:  ")];
    label_spans.extend(render_text_input(&d.label, is_label_active, base_style));

    let (color_name, color_fg) = match d.label_color {
        crate::session::LabelColor::Gray => ("gray", Color::DarkGray),
        crate::session::LabelColor::Magenta => ("magenta", Color::Magenta),
        crate::session::LabelColor::Cyan => ("cyan", Color::Cyan),
        crate::session::LabelColor::Green => ("green", Color::Green),
        crate::session::LabelColor::Yellow => ("yellow", Color::Yellow),
        crate::session::LabelColor::Red => ("red", Color::Red),
        crate::session::LabelColor::Blue => ("blue", Color::Blue),
    };

    let color_style = if d.field == crate::ui::SessionEditField::Color {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lines = vec![
        Line::from(Span::styled(
            "Edit Session",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(label_spans),
        Line::from(vec![
            Span::raw("Color:  "),
            Span::styled(
                format!("{color_name}"),
                color_style.fg(color_fg).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(":next field  "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(":next/apply  "),
            Span::styled("←/→", Style::default().fg(Color::Yellow)),
            Span::raw(":color  "),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(":cancel"),
        ]),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Session"));

    f.render_widget(p, popup_area);
}

fn render_rename_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameGroupDialog) {
    let popup_area = centered_rect(70, 35, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut new_path_spans = vec![Span::raw("To:    ")];
    new_path_spans.extend(render_text_input(&d.new_path, true, base_style));

    let lines = vec![
        Line::from(Span::styled(
            "Rename Group",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("From:  "),
            Span::styled(d.old_path.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(new_path_spans),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: apply • Esc/Ctrl+C: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Group"));

    f.render_widget(p, popup_area);
}

fn render_quit_confirm_dialog(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(40, 20, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            "Quit Agent Hand?",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Press q again to quit."),
        Line::from("Any other key to cancel."),
    ];

    let p = Paragraph::new(lines)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Confirm Quit"));

    f.render_widget(p, popup_area);
}

fn render_delete_confirm_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteConfirmDialog) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            "Delete session?",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Title: "),
            Span::styled(
                d.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("ID:    "),
            Span::styled(d.session_id.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Kill tmux session: "),
            Span::styled(
                if d.kill_tmux { "YES" } else { "NO" },
                if d.kill_tmux {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                },
            ),
            Span::raw("  (press 't' to toggle)"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "y/Enter: confirm • n/Esc/Ctrl+C: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Confirm"));

    f.render_widget(p, popup_area);
}

fn render_delete_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteGroupDialog) {
    let popup_area = centered_rect(70, 35, area);
    f.render_widget(Clear, popup_area);

    let active = Style::default().fg(Color::Black).bg(Color::Cyan);

    let opt1_style = if d.choice == crate::ui::DeleteGroupChoice::DeleteGroupKeepSessions {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let opt2_style = if d.choice == crate::ui::DeleteGroupChoice::Cancel {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let opt3_style = if d.choice == crate::ui::DeleteGroupChoice::DeleteGroupAndSessions {
        active
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let lines = vec![
        Line::from(Span::styled(
            "Delete group?",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Group: "),
            Span::styled(
                d.group_path.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Sessions: "),
            Span::styled(
                d.session_count.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("1", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("Delete group only (keep sessions)", opt1_style),
        ]),
        Line::from(vec![
            Span::styled("2", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("Cancel", opt2_style),
        ]),
        Line::from(vec![
            Span::styled("3", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled("Delete group + sessions", opt3_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "1/2/3 or ↑/↓ • Enter: confirm • Esc/Ctrl+C: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Confirm"));

    f.render_widget(p, popup_area);
}

fn render_search_popup(f: &mut Frame, area: Rect, app: &App) {
    let popup_area = centered_rect(80, 60, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Search",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::raw(format!(
        "Query: {}",
        app.search_query()
    ))));
    lines.push(Line::from(""));

    for (i, id) in app.search_results().iter().enumerate() {
        let s = app.session_by_id(id);
        let title = s.map(|x| x.title.as_str()).unwrap_or("<missing>");
        let group = s.map(|x| x.group_path.as_str()).unwrap_or("");
        let path = s
            .map(|x| x.project_path.to_string_lossy().to_string())
            .unwrap_or_default();

        let style = if i == app.search_selected() {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        lines.push(Line::from(vec![
            Span::styled(title.to_string(), style),
            Span::raw("  "),
            Span::styled(format!("[{}]", group), Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled(path, Style::default().fg(Color::DarkGray)),
        ]));
    }

    if app.search_results().is_empty() {
        lines.push(Line::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Type to filter • ↑/↓ to select • Enter to jump • Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Search"));

    f.render_widget(p, popup_area);
}

fn render_settings_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::SettingsDialog,
) {
    use crate::ui::{SettingsField, SettingsTab};

    let popup_area = centered_rect(70, 65, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let active_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        " Settings",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Tab bar
    let tabs = SettingsTab::available_tabs();
    let mut tab_spans: Vec<Span<'static>> = Vec::new();
    tab_spans.push(Span::raw(" "));
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled("  ", dim_style));
        }
        let label = format!(" {} ", tab.label());
        if *tab == d.tab {
            tab_spans.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(label, dim_style));
        }
    }
    lines.push(Line::from(tab_spans));
    lines.push(Line::from(Span::styled(
        " ─".to_string() + &"─".repeat(popup_area.width.saturating_sub(4) as usize),
        dim_style,
    )));
    lines.push(Line::from(""));

    // Fields for current tab
    let fields = SettingsField::fields_for_tab(d.tab);
    for field in &fields {
        let is_active = *field == d.field;
        let label = format!("  {:<16}", field.label());
        let label_style = if is_active { active_style } else { base_style };

        let mut spans: Vec<Span<'static>> = vec![Span::styled(label, label_style)];

        match field {
            SettingsField::AiProvider => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    // Expanded chip selector: show all providers with wrapping
                    let max_width = popup_area.width.saturating_sub(22) as usize;
                    let mut row_width = 0usize;
                    let mut first_in_row = true;
                    let mut overflow_lines: Vec<Vec<Span<'static>>> = Vec::new();
                    let current_spans = &mut spans;

                    for (i, name) in d.ai_provider_names.iter().enumerate() {
                        let chip = format!(" {} ", name);
                        let chip_len = chip.len() + 1;
                        if !first_in_row && row_width + chip_len > max_width {
                            overflow_lines.push(Vec::new());
                            row_width = 0;
                            first_in_row = true;
                        }

                        let style = if i == d.ai_provider_idx {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        };

                        let target = if overflow_lines.is_empty() {
                            &mut *current_spans
                        } else {
                            overflow_lines.last_mut().unwrap()
                        };
                        if !first_in_row {
                            target.push(Span::raw(" "));
                        }
                        target.push(Span::styled(chip, style));
                        row_width += chip_len;
                        first_in_row = false;
                    }

                    lines.push(Line::from(std::mem::take(current_spans)));
                    for row in overflow_lines {
                        let mut indented: Vec<Span<'static>> = vec![Span::raw(
                            " ".repeat(18),
                        )];
                        indented.extend(row);
                        lines.push(Line::from(indented));
                    }
                    continue;
                } else {
                    // Collapsed: show just the selected provider name
                    let val = format!("▸ {}", d.provider_display());
                    spans.push(Span::styled(val, if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::AiApiKey => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_api_key, true, base_style));
                } else {
                    let masked = d.masked_api_key();
                    let display = if masked.is_empty() {
                        "(not set)".to_string()
                    } else {
                        masked
                    };
                    spans.push(Span::styled(display, if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiModel => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_model, true, base_style));
                } else {
                    let t = d.ai_model.text();
                    let display = if t.is_empty() { "(provider default)" } else { t };
                    spans.push(Span::styled(display.to_string(), if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiBaseUrl => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_base_url, true, base_style));
                } else {
                    let t = d.ai_base_url.text();
                    let display = if t.is_empty() { "(provider default)" } else { t };
                    spans.push(Span::styled(display.to_string(), if is_active { active_style } else { dim_style }));
                }
            }
            SettingsField::AiSummaryLines => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.ai_summary_lines, true, base_style));
                } else {
                    spans.push(Span::styled(
                        d.ai_summary_lines.text().to_string(),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
            SettingsField::AiTest => {
                if let Some(status) = &d.ai_test_status {
                    let color = if status.starts_with('✓') {
                        Color::Green
                    } else if status.starts_with('✗') {
                        Color::Red
                    } else {
                        Color::Yellow
                    };
                    spans.push(Span::styled(status.clone(), Style::default().fg(color)));
                } else {
                    spans.push(Span::styled(
                        "[press Enter to test]",
                        if is_active { active_style } else { dim_style },
                    ));
                }
            }
            SettingsField::DefaultPermission => {
                let is_editing_this = d.editing && is_active;
                let is_ro = d.default_permission != "rw";
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Read Only ", if is_ro { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" Read/Write ", if !is_ro { sel } else { unsel }));
                } else {
                    let val = if is_ro { "Read Only" } else { "Read/Write" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            SettingsField::AnalyticsEnabled => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.analytics_enabled { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.analytics_enabled { sel } else { unsel }));
                } else {
                    let val = if d.analytics_enabled { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // Text input fields: relay_url, tmate_host, tmate_port, auto_expire, jump_lines, ready_ttl
            _ => {
                let input = match field {
                    SettingsField::RelayServerUrl => &d.relay_url,
                    SettingsField::TmateHost => &d.tmate_host,
                    SettingsField::TmatePort => &d.tmate_port,
                    SettingsField::AutoExpire => &d.auto_expire,
                    SettingsField::JumpLines => &d.jump_lines,
                    SettingsField::ReadyTtl => &d.ready_ttl,
                    _ => unreachable!(),
                };
                if d.editing && is_active {
                    spans.extend(render_text_input(input, true, base_style));
                } else {
                    let t = input.text();
                    let display = if t.is_empty() { "(not set)" } else { t };
                    spans.push(Span::styled(
                        display.to_string(),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
        }

        lines.push(Line::from(spans));
    }

    // Spacing + status
    lines.push(Line::from(""));

    // Dirty indicator
    if d.dirty {
        lines.push(Line::from(Span::styled(
            "  ● Unsaved changes",
            Style::default().fg(Color::Yellow),
        )));
    }

    // Key hints
    lines.push(Line::from(""));
    let hint_style = Style::default().fg(Color::DarkGray);
    if d.editing {
        let is_selector = matches!(
            d.field,
            SettingsField::AiProvider
                | SettingsField::DefaultPermission
                | SettingsField::AnalyticsEnabled
        );
        if is_selector {
            lines.push(Line::from(Span::styled(
                "  ←/→:choose  Enter/Esc:done",
                hint_style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  type to edit  Enter/Esc:done",
                hint_style,
            )));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ←/→:tab  ", hint_style),
            Span::styled("j/k:field  ", hint_style),
            Span::styled("Enter:edit  ", hint_style),
            Span::styled("Ctrl+S:save  ", hint_style),
            Span::styled("Esc:close", hint_style),
        ]));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Settings"));

    f.render_widget(p, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    let vertical = popup_layout[1];

    let popup_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical);

    popup_layout[1]
}

/// Render help as a centered modal overlay
fn render_help_modal(f: &mut Frame, area: Rect) {
    let modal_area = centered_rect(80, 85, area);
    f.render_widget(Clear, modal_area);

    let section = |label: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("── {label} "),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "─".repeat(40usize.saturating_sub(label.len() + 4)),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };

    let key = |k: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("  {:<12}", k),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(desc.to_string()),
        ])
    };

    let help_text: Vec<Line<'static>> = vec![
        Line::from(""),
        section("Navigation"),
        key("↑/k", "Move up"),
        key("↓/j", "Move down"),
        key("←/→", "Toggle group"),
        key("/", "Search"),
        Line::from(""),
        section("Session Actions"),
        key("Enter", "Attach"),
        key("s", "Start"),
        key("x", "Stop"),
        key("r", "Edit"),
        key("R", "Restart"),
        key("m", "Move to group"),
        key("f", "Fork"),
        key("d", "Delete"),
        key("b", "Boost active"),
        #[cfg(feature = "max")]
        key("A", "AI summary (Max)"),
        Line::from(""),
        section("Group Actions"),
        key("Enter", "Toggle"),
        key("r", "Rename"),
        key("d", "Delete"),
        Line::from(""),
        section("Global"),
        key("n", "New session"),
        key("g", "Create group"),
        key("p", "Preview snapshot"),
        key("Ctrl+r", "Refresh"),
        key("Ctrl+e", "Relationships"),
        key("S", "Share (Max)"),
        key("Tab", "Active panel (Pro)"),
        key(",", "Settings"),
        key("?", "Help"),
        key("q", "Quit"),
        Line::from(""),
        section("Status Indicators"),
        Line::from(vec![
            Span::styled("  !  ", Style::default().fg(Color::Blue)),
            Span::raw("WAITING"),
            Span::raw("    "),
            Span::styled("✓  ", Style::default().fg(Color::Cyan)),
            Span::raw("READY"),
            Span::raw("     "),
            Span::styled("●  ", Style::default().fg(Color::Yellow)),
            Span::raw("RUNNING"),
        ]),
        Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::raw("IDLE"),
            Span::raw("       "),
            Span::styled("✕  ", Style::default().fg(Color::Red)),
            Span::raw("ERROR"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "          Press ? or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(help_text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " ⌨ Commands & Shortcuts ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(help, modal_area);
}

/// Render status bar
/// Render keyboard hint spans for normal item selection (shared by free and pro builds)
fn render_item_hints(spans: &mut Vec<Span<'static>>, app: &App) {
    match app.selected_item() {
        Some(TreeItem::Group { .. }) => {
            spans.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":toggle  "));
            spans.push(Span::styled("r", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(":rename  "));
            spans.push(Span::styled("d", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":del  "));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":group+  "));
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":new  "));
        }
        Some(TreeItem::Session { .. }) => {
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":new  "));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":group+  "));
            spans.push(Span::styled("r", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(":rename  "));
            spans.push(Span::styled("R", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(":restart  "));
            spans.push(Span::styled("d", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":del  "));
            spans.push(Span::styled("f", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":fork  "));
            spans.push(Span::styled("m", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":move  "));
            spans.push(Span::styled("b", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":boost  "));
            #[cfg(feature = "max")]
            {
                spans.push(Span::styled("A", Style::default().fg(Color::Magenta)));
                spans.push(Span::raw(":AI  "));
            }
        }
        _ => {
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":new  "));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(":group+  "));
        }
    }
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let sessions = app.sessions();

    let waiting = sessions
        .iter()
        .filter(|s| s.status == Status::Waiting)
        .count();
    let attention = sessions
        .iter()
        .filter(|s| s.status == Status::Idle && app.is_attention_active(&s.id))
        .count();
    let running = sessions
        .iter()
        .filter(|s| s.status == Status::Running)
        .count();
    let idle = sessions
        .iter()
        .filter(|s| s.status == Status::Idle && !app.is_attention_active(&s.id))
        .count();

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            waiting_anim(app.tick_count()),
            Style::default().fg(Color::Blue),
        ),
        Span::raw(format!("{}", waiting)),
        Span::raw("  "),
        Span::styled("✓", Style::default().fg(Color::Cyan)),
        Span::raw(format!("{}", attention)),
        Span::raw("  "),
        Span::styled(
            running_anim(app.tick_count()),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(format!("{}", running)),
        Span::raw("  "),
        Span::styled("○", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}", idle)),
        Span::raw("  |  "),
    ];

    // PTY gauge: green < 50%, yellow 50-80%, red > 80%
    let pty_pct = if app.system_ptmx_max() > 0 {
        app.system_ptmx_total() as f32 / app.system_ptmx_max() as f32
    } else {
        0.0
    };
    let pty_color = if pty_pct < 0.5 {
        Color::Green
    } else if pty_pct < 0.8 {
        Color::Yellow
    } else {
        Color::Red
    };
    spans.push(Span::styled(
        format!("PTY: {}/{}", app.system_ptmx_total(), app.system_ptmx_max()),
        Style::default().fg(pty_color),
    ));
    spans.push(Span::raw("  |  "));

    #[cfg(feature = "pro")]
    if app.state() == crate::ui::AppState::Relationships {
        spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(":new  "));
        spans.push(Span::styled("d", Style::default().fg(Color::Red)));
        spans.push(Span::raw(":del  "));
        spans.push(Span::styled("c", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(":capture  "));
        spans.push(Span::styled("a", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(":annotate  "));
        spans.push(Span::styled("^N", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(":from-ctx  "));
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(":back"));
    } else {
        render_item_hints(&mut spans, app);
    }
    #[cfg(not(feature = "pro"))]
    render_item_hints(&mut spans, app);

    spans.push(Span::styled("/", Style::default().fg(Color::Cyan)));
    spans.push(Span::raw(":search  "));
    #[cfg(feature = "pro")]
    {
        spans.push(Span::styled("^E", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(":rels  "));
    }
    spans.push(Span::styled("p", Style::default().fg(Color::Cyan)));
    spans.push(Span::raw(":preview  "));
    spans.push(Span::styled("?", Style::default().fg(Color::Magenta)));
    spans.push(Span::raw(":help  "));
    spans.push(Span::styled("q", Style::default().fg(Color::Red)));
    spans.push(Span::raw(":quit"));

    if app.state() == crate::ui::AppState::Search {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            "Search: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(app.search_query().to_string()));
        spans.push(Span::raw(format!(" ({})", app.search_matches())));
    }

    // Tab hint for active panel (premium, only when there are active sessions)
    #[cfg(feature = "pro")]
    {
        let is_pro = app.auth_token().map_or(false, |t| t.is_pro());
        let has_active = !app.active_sessions().is_empty();
        if is_pro && has_active {
            spans.push(Span::raw("  |  "));
            spans.push(Span::styled("Tab", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(":active-panel"));
        }
    }

    // User account badge
    spans.push(Span::raw("  |  "));
    if let Some(token) = app.auth_token() {
        #[cfg(feature = "max")]
        let is_max_badge = token.is_max();
        #[cfg(not(feature = "max"))]
        let is_max_badge = false;

        if is_max_badge {
            spans.push(Span::styled(
                "MAX",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if token.is_pro() {
            spans.push(Span::styled(
                "PRO",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("FREE", Style::default().fg(Color::DarkGray)));
        }
        let email_display = token.email.split('@').next().unwrap_or(&token.email);
        spans.push(Span::raw(format!(" {}", email_display)));
    } else {
        spans.push(Span::styled("not logged in", Style::default().fg(Color::DarkGray)));
    }

    let status_line = Line::from(spans);

    let status = Paragraph::new(status_line).block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

/// Render the Relationships view (Premium)
#[cfg(feature = "pro")]
fn render_relationships(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let relationships = app.relationships();
    let selected = app.selected_relationship_index();

    // Left panel: relationship list with selection
    let items: Vec<ListItem> = if relationships.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  No relationships yet. Press 'n' to create one.",
            Style::default().fg(Color::DarkGray),
        )]))]
    } else {
        relationships
            .iter()
            .enumerate()
            .map(|(i, rel)| {
                let a_title = app
                    .session_by_id(&rel.session_a_id)
                    .map(|s| s.title.as_str())
                    .unwrap_or("?");
                let b_title = app
                    .session_by_id(&rel.session_b_id)
                    .map(|s| s.title.as_str())
                    .unwrap_or("?");
                let indicator = rel.direction_indicator();
                let label = rel
                    .label
                    .as_deref()
                    .map(|l| format!(" \"{}\"", l))
                    .unwrap_or_default();

                let is_selected = i == selected;
                let marker = if is_selected { "▸ " } else { "  " };
                let style = if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(marker.to_string(), style),
                    Span::styled(a_title.to_string(), Style::default().fg(Color::Cyan)),
                    Span::raw(format!(" {} ", indicator)),
                    Span::styled(b_title.to_string(), Style::default().fg(Color::Cyan)),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", rel.relation_type),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(label, Style::default().fg(Color::Yellow)),
                    // Dependency satisfaction indicator
                    if rel.relation_type == crate::session::RelationType::Dependency {
                        let source_idle = app
                            .session_by_id(&rel.session_a_id)
                            .is_some_and(|s| matches!(s.status, crate::session::Status::Idle));
                        if source_idle {
                            Span::styled(" ✓ ready", Style::default().fg(Color::Green))
                        } else {
                            Span::styled(" ⏳", Style::default().fg(Color::Yellow))
                        }
                    } else {
                        Span::raw("")
                    },
                ]);
                ListItem::new(line).style(if is_selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                })
            })
            .collect()
    };

    let mut list_state = ListState::default();
    if !relationships.is_empty() {
        list_state.select(Some(selected));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Relationships ({})", relationships.len())),
    );
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Right panel: context preview
    let preview_text = if relationships.is_empty() {
        "Select a relationship to see context.\n\n\
         Ctrl+E: back to sessions\n\
         n: new relationship\n\
         d: delete relationship\n\
         c: capture context\n\
         a: annotate"
            .to_string()
    } else if let Some(rel) = relationships.get(selected) {
        let a_title = app
            .session_by_id(&rel.session_a_id)
            .map(|s| s.title.as_str())
            .unwrap_or("?");
        let b_title = app
            .session_by_id(&rel.session_b_id)
            .map(|s| s.title.as_str())
            .unwrap_or("?");
        let a_status = app
            .session_by_id(&rel.session_a_id)
            .map(|s| format!("{:?}", s.status))
            .unwrap_or_else(|| "Unknown".to_string());
        let b_status = app
            .session_by_id(&rel.session_b_id)
            .map(|s| format!("{:?}", s.status))
            .unwrap_or_else(|| "Unknown".to_string());
        let label_line = rel
            .label
            .as_deref()
            .map(|l| format!("Label: \"{}\"\n", l))
            .unwrap_or_default();

        // Dependency satisfaction info
        let dep_info = if rel.relation_type == crate::session::RelationType::Dependency {
            let source_idle = app
                .session_by_id(&rel.session_a_id)
                .is_some_and(|s| matches!(s.status, crate::session::Status::Idle));
            if source_idle {
                "\n✓ Dependency satisfied — source session is idle.\n  Output may be ready for consumption.\n".to_string()
            } else {
                "\n⏳ Dependency pending — source session still active.\n".to_string()
            }
        } else {
            String::new()
        };

        let snap_count = app.snapshot_count(&rel.id);
        let snap_info = if snap_count > 0 {
            format!("\nSnapshots: {}\n", snap_count)
        } else {
            "\nNo snapshots captured yet.\n".to_string()
        };

        format!(
            "=== {} ===\n\
             Type: {}\n\
             {}\n\
             Session A: \"{}\"\n\
             Status: {}\n\n\
             Session B: \"{}\"\n\
             Status: {}\n\
             {}{}\n\
             [c] Capture  [a] Annotate  [Ctrl+N] New from ctx\n\
             [d] Delete   [Esc] Back",
            rel.direction_indicator(),
            rel.relation_type,
            label_line,
            a_title,
            a_status,
            b_title,
            b_status,
            dep_info,
            snap_info,
        )
    } else {
        "No relationship selected.".to_string()
    };

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Context Preview"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[1]);
}

#[cfg(feature = "pro")]
fn render_join_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::JoinSessionDialog) {
    let popup_area = centered_rect(65, 30, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Join Shared Session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Rgb(20, 20, 35)));
    f.render_widget(block, popup_area);

    let inner = popup_area.inner(Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // status
            Constraint::Length(1), // spacer
            Constraint::Length(1), // hint
        ])
        .split(inner);

    let label = Paragraph::new("Paste share URL:")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(label, chunks[0]);

    let input_text = d.url_input.text();
    let input = Paragraph::new(format!("▸ {}", input_text))
        .style(Style::default().fg(Color::White));
    f.render_widget(input, chunks[1]);

    if let Some(ref status) = d.status {
        let color = if status.contains("Invalid") || status.contains("fail") {
            Color::Red
        } else {
            Color::Yellow
        };
        let status_line = Paragraph::new(status.as_str())
            .style(Style::default().fg(color));
        f.render_widget(status_line, chunks[3]);
    }

    let hint = Paragraph::new("Enter: connect  Esc: cancel")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[5]);
}

#[cfg(feature = "pro")]
fn render_share_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ShareDialog) {
    let popup_area = centered_rect(65, 50, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!("Share: {}", d.session_title));

    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Permission
            Constraint::Length(2), // Status
            Constraint::Length(2), // SSH URL
            Constraint::Length(2), // Web URL
            Constraint::Length(2), // Expire
            Constraint::Min(1),   // Actions
        ])
        .split(inner_area);

    // Permission line
    let perm_text = format!("Permission: {} (Tab to toggle)", d.permission);
    f.render_widget(
        Paragraph::new(perm_text).style(Style::default().fg(Color::White)),
        chunks[0],
    );

    // Status
    let status = if d.already_sharing {
        Span::styled(
            "● Sharing active",
            Style::default().fg(Color::Green),
        )
    } else {
        Span::styled(
            "○ Not sharing",
            Style::default().fg(Color::DarkGray),
        )
    };
    f.render_widget(Paragraph::new(Line::from(status)), chunks[1]);

    // URL display — prefer relay URL over SSH/web
    if let Some(ref relay_url) = d.relay_share_url {
        // Relay mode
        let relay_line = format!("Share URL: {} (press 'c' to copy)", relay_url);
        f.render_widget(
            Paragraph::new(relay_line).style(Style::default().fg(Color::Cyan)),
            chunks[2],
        );
        f.render_widget(
            Paragraph::new("Mode: WebSocket relay")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[3],
        );
    } else {
        // Tmate mode
        let ssh_line = if let Some(ref url) = d.ssh_url {
            format!("SSH: {} (press 'c' to copy)", url)
        } else {
            "SSH: -".to_string()
        };
        f.render_widget(
            Paragraph::new(ssh_line).style(Style::default().fg(Color::Cyan)),
            chunks[2],
        );

        let web_line = if let Some(ref url) = d.web_url {
            format!("Web: {}", url)
        } else {
            "Web: -".to_string()
        };
        f.render_widget(
            Paragraph::new(web_line).style(Style::default().fg(Color::Cyan)),
            chunks[3],
        );
    }

    // Expire minutes input
    let mut expire_spans = vec![Span::raw("Expire (min): ")];
    expire_spans.extend(render_text_input(&d.expire_minutes, true, Style::default()));
    f.render_widget(Paragraph::new(Line::from(expire_spans)), chunks[4]);

    // Actions hint
    let action = if d.already_sharing {
        "Enter: Stop sharing  |  c: Copy URL  |  Esc: Close"
    } else {
        "Enter: Start sharing  |  Esc: Close"
    };
    f.render_widget(
        Paragraph::new(action)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

#[cfg(feature = "pro")]
fn render_create_relationship_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::CreateRelationshipDialog,
) {
    let popup_area = centered_rect(65, 55, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title("New Relationship");
    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let is_search = d.field == crate::ui::CreateRelationshipField::Search;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // From session
            Constraint::Length(2), // Type
            Constraint::Length(3), // Search
            Constraint::Min(3),   // Matches
            Constraint::Length(3), // Label
            Constraint::Length(2), // Actions
        ])
        .split(inner_area);

    // From session
    f.render_widget(
        Paragraph::new(format!("From: {}", d.session_a_title))
            .style(Style::default().fg(Color::Cyan)),
        chunks[0],
    );

    // Relation type
    f.render_widget(
        Paragraph::new(format!("Type: {} (Tab to cycle)", d.relation_type))
            .style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );

    // Search input
    let mut search_spans = vec![Span::raw("Search: ")];
    search_spans.extend(render_text_input(
        &d.search_input,
        is_search,
        Style::default(),
    ));
    f.render_widget(
        Paragraph::new(Line::from(search_spans))
            .block(Block::default().borders(Borders::ALL).title(if is_search {
                "Search (active)"
            } else {
                "Search"
            })),
        chunks[2],
    );

    // Session matches
    let items: Vec<ListItem> = if d.matches.is_empty() {
        vec![ListItem::new(Span::styled(
            "  No matching sessions",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        d.matches
            .iter()
            .enumerate()
            .map(|(i, (_id, title))| {
                let marker = if i == d.selected { "▸ " } else { "  " };
                let style = if i == d.selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(format!("{}{}", marker, title), style))
            })
            .collect()
    };
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Select Target Session"),
    );
    f.render_widget(list, chunks[3]);

    // Label input
    let mut label_spans = vec![Span::raw("Label: ")];
    label_spans.extend(render_text_input(
        &d.label,
        !is_search,
        Style::default(),
    ));
    f.render_widget(
        Paragraph::new(Line::from(label_spans)).block(
            Block::default().borders(Borders::ALL).title(if !is_search {
                "Label (active)"
            } else {
                "Label (optional)"
            }),
        ),
        chunks[4],
    );

    // Actions
    f.render_widget(
        Paragraph::new("Enter: Create  |  Tab: Cycle type  |  Shift+Tab: Switch field  |  Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

#[cfg(feature = "pro")]
fn render_annotate_dialog(f: &mut Frame, area: Rect, d: &crate::ui::AnnotateDialog) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title("Annotate Relationship");
    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner_area);

    f.render_widget(
        Paragraph::new("Add a note to this relationship:")
            .style(Style::default().fg(Color::White)),
        chunks[0],
    );

    let note_spans = render_text_input(&d.note, true, Style::default());
    f.render_widget(
        Paragraph::new(Line::from(note_spans))
            .block(Block::default().borders(Borders::ALL).title("Note")),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new("Enter: Save  |  Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
    );
}

#[cfg(feature = "pro")]
fn render_new_from_context_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::NewFromContextDialog,
) {
    let popup_area = centered_rect(70, 60, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title("New Session from Context");
    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(2), // Injection method
            Constraint::Min(3),   // Context preview
            Constraint::Length(2), // Actions
        ])
        .split(inner_area);

    // Title input
    let title_spans = render_text_input(&d.title, true, Style::default());
    f.render_widget(
        Paragraph::new(Line::from(title_spans))
            .block(Block::default().borders(Borders::ALL).title("Session Title")),
        chunks[0],
    );

    // Injection method
    f.render_widget(
        Paragraph::new(format!("Injection: {} (Tab to cycle)", d.injection_method))
            .style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );

    // Context preview
    f.render_widget(
        Paragraph::new(d.context_preview.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Context Preview"),
            )
            .wrap(Wrap { trim: false }),
        chunks[2],
    );

    // Actions
    f.render_widget(
        Paragraph::new("Enter: Create  |  Tab: Cycle method  |  Esc: Cancel")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[3],
    );
}

/// Render the viewer mode — displays a shared terminal session received via relay.
#[cfg(feature = "pro")]
fn render_viewer_mode(f: &mut Frame, area: Rect, app: &App) {
    let Some(vs) = app.viewer_state() else {
        // No viewer state — show placeholder
        let msg = Paragraph::new("Not connected to any shared session.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Terminal content
            Constraint::Length(1), // Presence bar
        ])
        .split(area);

    // Render terminal content
    // We display the raw text content. The content includes ANSI escapes,
    // but ratatui's Paragraph will render them as-is (plain text).
    // For a full terminal emulator we'd need a vt100 parser, but plain text
    // display is a good starting point that shows the content.
    let content = vs.terminal_content.blocking_lock();
    let text = String::from_utf8_lossy(&content);

    // Take last N lines that fit in the area
    let lines: Vec<&str> = text.lines().collect();
    let visible_height = chunks[0].height as usize;
    let start = lines.len().saturating_sub(visible_height);
    let visible_lines: Vec<Line> = lines[start..]
        .iter()
        .map(|line| {
            // Strip ANSI escape codes for clean display
            let cleaned = strip_ansi_escapes(line);
            Line::from(cleaned)
        })
        .collect();

    let terminal_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", vs.session_name));

    let terminal_paragraph = Paragraph::new(visible_lines)
        .block(terminal_block)
        .style(Style::default().fg(Color::White));

    f.render_widget(terminal_paragraph, chunks[0]);

    // Render presence bar
    let connected = vs.connected.load(std::sync::atomic::Ordering::Relaxed);
    let viewer_count = vs.viewer_count.load(std::sync::atomic::Ordering::Relaxed);

    let status_text = if connected {
        format!(
            "  Viewing {}  |  {} viewer{}  |  Press Esc to disconnect",
            vs.session_name,
            viewer_count,
            if viewer_count == 1 { "" } else { "s" }
        )
    } else {
        format!("  Disconnected from {}  |  Press Esc to return", vs.session_name)
    };

    let status_color = if connected { Color::Green } else { Color::Red };
    let presence_bar = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White).bg(status_color));

    f.render_widget(presence_bar, chunks[1]);
}

/// Strip ANSI escape sequences from a string for clean display.
#[cfg(feature = "pro")]
fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ESC sequence
            if let Some(&'[') = chars.peek() {
                chars.next(); // consume '['
                // Read until we hit a letter (the terminating character)
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() || c == 'm' || c == 'H' || c == 'J' || c == 'K' {
                        break;
                    }
                }
            }
        } else if ch.is_control() && ch != '\n' && ch != '\t' {
            // Skip other control characters
        } else {
            result.push(ch);
        }
    }

    result
}
