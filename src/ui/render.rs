use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::session::Status;

use super::app::App;
use super::TreeItem;

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

    // Render content
    if app.help_visible() {
        render_help(f, chunks[1]);
    } else {
        render_main(f, chunks[1], app);
    }

    // Render status bar
    render_status_bar(f, chunks[2], app);

    if app.state() == crate::ui::AppState::Dialog {
        render_dialog(f, f.area(), app);
    }

    if app.state() == crate::ui::AppState::Search {
        render_search_popup(f, f.area(), app);
    }
}

/// Render title bar
fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("🦀 Agent Deck (Rust)")
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

/// Render session list
fn render_session_list(f: &mut Frame, area: Rect, app: &App) {
    let tree = app.tree();

    if tree.is_empty() {
        let empty = Paragraph::new("No sessions found.\n\nUse: agent-deck add ...\nPress 'n' to create.\nPress '?' for help.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Sessions"));

        f.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = tree
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.selected_index();
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

                    let (status_icon, status_color, title, tool) = if let Some(session) = s {
                        let status_icon = match session.status {
                            Status::Waiting => "⏸",
                            Status::Running => "▶",
                            Status::Idle => "○",
                            Status::Error => "✕",
                            Status::Starting => "⋯",
                        };

                        let status_color = match session.status {
                            Status::Waiting => Color::Yellow,
                            Status::Running => Color::Green,
                            Status::Idle => Color::DarkGray,
                            Status::Error => Color::Red,
                            Status::Starting => Color::Cyan,
                        };

                        (
                            status_icon,
                            status_color,
                            session.title.as_str(),
                            session.tool.to_string(),
                        )
                    } else {
                        ("?", Color::Red, "<missing>", "".to_string())
                    };

                    let line = Line::from(vec![
                        Span::styled(indent, Style::default()),
                        Span::styled(status_icon, Style::default().fg(status_color)),
                        Span::raw(" "),
                        Span::styled(title, base.add_modifier(Modifier::BOLD)),
                        Span::raw(" "),
                        Span::styled(format!("({})", tool), Style::default().fg(Color::DarkGray)),
                    ]);
                    ListItem::new(line)
                }
            }
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
        "Tree ({}/{})",
        app.selected_index() + 1,
        tree.len()
    )));

    f.render_widget(list, area);
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
    if let Some(d) = app.new_session_dialog() {
        render_new_session_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.delete_confirm_dialog() {
        render_delete_confirm_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.mcp_dialog() {
        render_mcp_dialog(f, area, d);
        return;
    }

    if let Some(d) = app.fork_dialog() {
        render_fork_dialog(f, area, d);
    }
}

fn render_new_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::NewSessionDialog) {
    let popup_area = centered_rect(70, 50, area);
    f.render_widget(Clear, popup_area);

    let active_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let path_style = if d.field == crate::ui::NewSessionField::Path {
        active_style
    } else {
        Style::default()
    };
    let title_style = if d.field == crate::ui::NewSessionField::Title {
        active_style
    } else {
        Style::default()
    };
    let tool_style = if d.field == crate::ui::NewSessionField::Tool {
        active_style
    } else {
        Style::default()
    };
    let cmd_style = if d.field == crate::ui::NewSessionField::Command {
        active_style
    } else {
        Style::default()
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "New Session",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Path:   "),
            Span::styled(d.path.clone(), path_style),
        ]),
    ];

    if d.path_suggestions_visible && !d.path_suggestions.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("Suggestions:", Style::default().fg(Color::DarkGray)),
        ]));
        for (i, s) in d.path_suggestions.iter().take(8).enumerate() {
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

    lines.extend([
        Line::from(vec![
            Span::raw("Title:  "),
            Span::styled(d.title.clone(), title_style),
        ]),
        Line::from(vec![
            Span::raw("Tool:   "),
            Span::styled(d.tool.as_str(), tool_style),
        ]),
        Line::from(vec![
            Span::raw("        "),
            Span::styled("Tools: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "claude ",
                if d.tool == crate::ui::NewSessionTool::Claude {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "gemini ",
                if d.tool == crate::ui::NewSessionTool::Gemini {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "opencode ",
                if d.tool == crate::ui::NewSessionTool::OpenCode {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "codex ",
                if d.tool == crate::ui::NewSessionTool::Codex {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "shell ",
                if d.tool == crate::ui::NewSessionTool::Shell {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "custom",
                if d.tool == crate::ui::NewSessionTool::Custom {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]),
        Line::from(vec![
            Span::raw("Cmd:    "),
            Span::styled(d.command.clone(), cmd_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: complete path (Path) / cycle tool (Tool) / next field • Shift-Tab: prev/cycle",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Enter: apply suggestion / next / submit • ←/→/↑/↓: tool • Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ]);

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("New"));

    f.render_widget(p, popup_area);
}

fn render_fork_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ForkDialog) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let active_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let title_style = if d.field == crate::ui::ForkField::Title {
        active_style
    } else {
        Style::default()
    };
    let group_style = if d.field == crate::ui::ForkField::Group {
        active_style
    } else {
        Style::default()
    };

    let lines = vec![
        Line::from(Span::styled(
            "Fork Session",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("Title: "),
            Span::styled(d.title.clone(), title_style),
        ]),
        Line::from(vec![
            Span::raw("Group: "),
            Span::styled(d.group_path.clone(), group_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab: switch field • Enter: next/submit • Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Fork"));

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
            "y/Enter: confirm • n/Esc: cancel",
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

fn render_mcp_dialog(f: &mut Frame, area: Rect, d: &crate::ui::MCPDialog) {
    let popup_area = centered_rect(85, 65, area);
    f.render_widget(Clear, popup_area);

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(popup_area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[0]);

    let left_title = if d.column == crate::ui::MCPColumn::Attached {
        "Attached (Enter to detach)"
    } else {
        "Attached"
    };

    let right_title = if d.column == crate::ui::MCPColumn::Available {
        "Available (Enter to attach)"
    } else {
        "Available"
    };

    let attached_items: Vec<ListItem> = if d.attached.is_empty() {
        vec![ListItem::new(Span::styled(
            "(none)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        d.attached
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if d.column == crate::ui::MCPColumn::Attached && i == d.attached_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(name.clone(), style))
            })
            .collect()
    };

    let available_items: Vec<ListItem> = if d.available.is_empty() {
        vec![ListItem::new(Span::styled(
            "(none)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        d.available
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if d.column == crate::ui::MCPColumn::Available && i == d.available_idx {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Span::styled(name.clone(), style))
            })
            .collect()
    };

    let left =
        List::new(attached_items).block(Block::default().borders(Borders::ALL).title(left_title));
    let right =
        List::new(available_items).block(Block::default().borders(Borders::ALL).title(right_title));

    f.render_widget(left, cols[0]);
    f.render_widget(right, cols[1]);

    let hint = Paragraph::new(
        "Tab: switch column • ↑/↓: move • Enter: toggle • a: apply(restart) • Esc: close",
    )
    .style(Style::default().fg(Color::DarkGray))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(hint, outer[1]);
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

/// Render help screen
fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑/k", Style::default().fg(Color::Yellow)),
            Span::raw("      Move selection up"),
        ]),
        Line::from(vec![
            Span::styled("  ↓/j", Style::default().fg(Color::Yellow)),
            Span::raw("      Move selection down"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Green)),
            Span::raw("    Attach to session / Toggle group"),
        ]),
        Line::from(vec![
            Span::styled("  ←/→/Space", Style::default().fg(Color::Yellow)),
            Span::raw(" Toggle group expand/collapse"),
        ]),
        Line::from(vec![
            Span::styled("  s", Style::default().fg(Color::Green)),
            Span::raw("        Start session"),
        ]),
        Line::from(vec![
            Span::styled("  x", Style::default().fg(Color::Red)),
            Span::raw("        Stop session"),
        ]),
        Line::from(vec![
            Span::styled("  r", Style::default().fg(Color::Yellow)),
            Span::raw("        Restart session"),
        ]),
        Line::from(vec![
            Span::styled("  n", Style::default().fg(Color::Cyan)),
            Span::raw("        New session"),
        ]),
        Line::from(vec![
            Span::styled("  d", Style::default().fg(Color::Cyan)),
            Span::raw("        Delete session"),
        ]),
        Line::from(vec![
            Span::styled("  m", Style::default().fg(Color::Cyan)),
            Span::raw("        MCP manager"),
        ]),
        Line::from(vec![
            Span::styled("  f", Style::default().fg(Color::Cyan)),
            Span::raw("        Fork session"),
        ]),
        Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::Cyan)),
            Span::raw("        Search"),
        ]),
        Line::from(vec![
            Span::styled("  p", Style::default().fg(Color::Cyan)),
            Span::raw("        Capture preview snapshot"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  R", Style::default().fg(Color::Cyan)),
            Span::raw("        Refresh"),
        ]),
        Line::from(vec![
            Span::styled("  ?", Style::default().fg(Color::Magenta)),
            Span::raw("        Toggle help"),
        ]),
        Line::from(vec![
            Span::styled("  q", Style::default().fg(Color::Red)),
            Span::raw("        Quit"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Status Indicators",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⏸ ", Style::default().fg(Color::Yellow)),
            Span::raw("  WAITING  - Agent waiting for input"),
        ]),
        Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Green)),
            Span::raw("  RUNNING  - Agent is busy"),
        ]),
        Line::from(vec![
            Span::styled("  ○ ", Style::default().fg(Color::DarkGray)),
            Span::raw("  IDLE     - Session not started"),
        ]),
        Line::from(vec![
            Span::styled("  ✕ ", Style::default().fg(Color::Red)),
            Span::raw("  ERROR    - Session error"),
        ]),
        Line::from(""),
    ];

    let help = Paragraph::new(help_text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Help"));

    f.render_widget(help, area);
}

/// Render status bar
fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let sessions = app.sessions();

    let waiting = sessions
        .iter()
        .filter(|s| s.status == Status::Waiting)
        .count();
    let running = sessions
        .iter()
        .filter(|s| s.status == Status::Running)
        .count();
    let idle = sessions.iter().filter(|s| s.status == Status::Idle).count();

    let mut spans = vec![
        Span::raw("  "),
        Span::styled("⏸", Style::default().fg(Color::Yellow)),
        Span::raw(format!("{}", waiting)),
        Span::raw("  "),
        Span::styled("▶", Style::default().fg(Color::Green)),
        Span::raw(format!("{}", running)),
        Span::raw("  "),
        Span::styled("○", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}", idle)),
        Span::raw("  |  "),
        Span::styled("n", Style::default().fg(Color::Cyan)),
        Span::raw(":new  "),
        Span::styled("d", Style::default().fg(Color::Cyan)),
        Span::raw(":del  "),
        Span::styled("m", Style::default().fg(Color::Cyan)),
        Span::raw(":mcp  "),
        Span::styled("f", Style::default().fg(Color::Cyan)),
        Span::raw(":fork  "),
        Span::styled("/", Style::default().fg(Color::Cyan)),
        Span::raw(":search  "),
        Span::styled("p", Style::default().fg(Color::Cyan)),
        Span::raw(":preview  "),
        Span::styled("?", Style::default().fg(Color::Magenta)),
        Span::raw(":help  "),
        Span::styled("q", Style::default().fg(Color::Red)),
        Span::raw(":quit"),
    ];

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

    let status_line = Line::from(spans);

    let status = Paragraph::new(status_line).block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}
