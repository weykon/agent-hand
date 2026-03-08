use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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

#[cfg(feature = "pro")]
fn connection_pulse(tick: u64) -> &'static str {
    // Subtle pulse for active connections
    const FRAMES: [&str; 4] = ["◉", "◉", "○", "○"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

/// Parse hex color string (e.g., "#3b82f6") to ratatui Color.
#[cfg(feature = "pro")]
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(feature = "pro")]
fn connection_quality_icon(latency_ms: u32) -> &'static str {
    // Quality indicator: excellent/good/poor
    if latency_ms < 50 {
        "●" // Excellent
    } else if latency_ms < 150 {
        "◐" // Good
    } else {
        "○" // Poor
    }
}

#[cfg(feature = "pro")]
fn format_bandwidth(bytes_per_sec: u64) -> String {
    // Format bandwidth: B/s, KB/s, MB/s
    if bytes_per_sec < 1024 {
        format!("{}B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1}KB/s", bytes_per_sec as f64 / 1024.0)
    } else {
        format!("{:.1}MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    }
}

/// Minimum visible width for input fields (in characters)
const INPUT_MIN_WIDTH: usize = 30;

/// Render a TextInput with cursor visible when active.
/// A subtle background strip marks the editable area so users can see where to type.
fn render_text_input(input: &TextInput, active: bool, _base_style: Style) -> Vec<Span<'static>> {
    let text = input.text();
    let cursor_pos = input.cursor();

    // Background strip so the input area is visually distinct
    let field_bg = if active {
        Color::DarkGray
    } else {
        Color::Indexed(236) // very subtle dark gray (#303030)
    };
    let field_style = Style::default().bg(field_bg);

    // Padding to fill the input area to a minimum visible width
    let text_char_len = text.chars().count();
    let pad_len = INPUT_MIN_WIDTH.saturating_sub(text_char_len + 1); // +1 for cursor
    let padding: String = " ".repeat(pad_len);

    if !active {
        let display = format!("{}{}", text, " ".repeat(INPUT_MIN_WIDTH.saturating_sub(text_char_len)));
        return vec![Span::styled(display, field_style)];
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
        Span::styled(before.to_string(), field_style),
        Span::styled(cursor_char.to_string(), cursor_style),
        Span::styled(format!("{}{}", rest, padding), field_style),
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
    render_title(f, chunks[0], app.language());

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
        render_help_modal(f, f.area(), app.language());
    }

    if app.state() == crate::ui::AppState::Dialog {
        render_dialog(f, f.area(), app);
    }

    if app.state() == crate::ui::AppState::Search {
        render_search_popup(f, f.area(), app);
    }

    // Toast notifications overlay (top-right corner)
    #[cfg(feature = "pro")]
    render_toast_notifications(f, f.area(), app);

    // Onboarding welcome message
    if app.show_onboarding() {
        render_onboarding_welcome(f, f.area(), app.language());
    }
}

/// Render title bar
fn render_title(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
    use crate::i18n::{Translate, Language};

    let title_text = match lang {
        Language::Chinese => "🦀 Agent Deck (Rust) 智能助手",
        Language::English => "🦀 Agent Deck (Rust) Agent Hand",
    };
    let help_hint = crate::i18n::ui::HelpHint.t(lang);

    let title_line = Line::from(vec![
        Span::styled(
            title_text.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {help_hint}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
        ),
    ]);

    let title = Paragraph::new(title_line)
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
        let has_viewer_sessions = !app.viewer_sessions().is_empty();

        if is_pro && (!active.is_empty() || has_viewer_sessions) {
            // Calculate heights for active panel and viewer sessions panel
            let max_h = (area.height * 2 / 5).max(8);
            let active_panel_h = if !active.is_empty() {
                (active.len() as u16 + 2).min(max_h)
            } else {
                0
            };
            let viewer_panel_h = if has_viewer_sessions {
                (app.viewer_sessions().len() as u16 + 2).min(max_h)
            } else {
                0
            };

            let mut constraints = vec![];
            if active_panel_h > 0 {
                constraints.push(Constraint::Length(active_panel_h));
            }
            if viewer_panel_h > 0 {
                constraints.push(Constraint::Length(viewer_panel_h));
            }
            constraints.push(Constraint::Min(0));

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            let mut row_idx = 0;
            if active_panel_h > 0 {
                render_active_panel(f, rows[row_idx], app, &active);
                row_idx += 1;
            }
            if viewer_panel_h > 0 {
                render_viewer_sessions_panel(f, rows[row_idx], app);
                row_idx += 1;
            }
            render_session_tree(f, rows[row_idx], app);
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
                Status::Idle => {
                    if app.is_attention_active(&s.id) {
                        "✓"
                    } else {
                        "○"
                    }
                }
            };
            let status_color = match s.status {
                Status::Waiting => Color::Blue,
                Status::Running => Color::Yellow,
                Status::Error => Color::Red,
                Status::Starting => Color::Cyan,
                Status::Idle => {
                    if app.is_attention_active(&s.id) {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }
                }
            };

            let mut spans = vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(s.title.clone(), base.add_modifier(Modifier::BOLD)),
            ];

            // Show sharing indicator with viewer count
            #[cfg(feature = "pro")]
            if let Some(ref sharing) = s.sharing {
                if sharing.active {
                    if let Some(relay) = app.relay_client(&s.id) {
                        let vc = relay.viewer_count();
                        if vc > 0 {
                            let viewers = relay.viewers();
                            let rw_viewer = viewers.iter().find(|v| v.permission == "rw");
                            if let Some(rw) = rw_viewer {
                                let name = truncate_name(&rw.display_name, 8);
                                spans.push(Span::styled(
                                    format!(" {}v {}", vc, name),
                                    if is_selected { base } else { Style::default().fg(Color::Cyan) },
                                ));
                            } else {
                                spans.push(Span::styled(
                                    format!(" {}v", vc),
                                    if is_selected { base } else { Style::default().fg(Color::Green) },
                                ));
                            }
                        } else {
                            spans.push(Span::styled(
                                " shared",
                                if is_selected { base } else { Style::default().fg(Color::DarkGray) },
                            ));
                        }
                    } else {
                        spans.push(Span::styled(
                            " shared",
                            if is_selected { base } else { Style::default().fg(Color::DarkGray) },
                        ));
                    }
                }
            }

            let line = Line::from(spans);
            ListItem::new(line)
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = match app.language() {
        crate::i18n::Language::Chinese => format!("⚡ 活跃会话 ({})", active.len()),
        crate::i18n::Language::English => format!("⚡ Active ({})", active.len()),
    };
    let list = List::new(items)
        .scroll_padding(app.scroll_padding())
        .block(
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

/// Render the viewer sessions panel (premium feature)
#[cfg(feature = "pro")]
fn render_viewer_sessions_panel(f: &mut Frame, area: Rect, app: &App) {
    let sessions = app.viewer_sessions();
    let focused = app.viewer_panel_focused();
    let selected = app.viewer_panel_selected();

    // Sort by connected_at so display order is deterministic
    let mut sorted: Vec<_> = sessions.iter().collect();
    sorted.sort_by_key(|(_, info)| info.connected_at);

    let items: Vec<ListItem> = sorted
        .iter()
        .enumerate()
        .map(|(i, (room_id, info))| {
            let is_selected = focused && i == selected;
            let base = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };

            let status_icon = match info.status {
                crate::ui::app::ViewerSessionStatus::Connecting => "⟳",
                crate::ui::app::ViewerSessionStatus::Connected => "●",
                crate::ui::app::ViewerSessionStatus::Disconnected => "○",
                crate::ui::app::ViewerSessionStatus::Reconnecting => "⟳",
            };

            let status_color = match info.status {
                crate::ui::app::ViewerSessionStatus::Connected => Color::Green,
                crate::ui::app::ViewerSessionStatus::Connecting |
                crate::ui::app::ViewerSessionStatus::Reconnecting => Color::Yellow,
                crate::ui::app::ViewerSessionStatus::Disconnected => Color::Red,
            };

            // Truncate room_id for display
            let display_room = if room_id.len() > 12 {
                format!("{}...", &room_id[..12])
            } else {
                room_id.to_string()
            };

            // Truncate relay_url for display
            let relay_display = info.relay_url
                .replace("https://", "")
                .replace("http://", "");
            let relay_display = if relay_display.len() > 30 {
                format!("{}...", &relay_display[..30])
            } else {
                relay_display
            };

            let line = Line::from(vec![
                Span::styled(status_icon, if is_selected { base } else { Style::default().fg(status_color) }),
                Span::raw(" "),
                Span::styled(display_room, if is_selected { base } else { Style::default().fg(Color::Cyan) }),
                Span::raw(" - "),
                Span::styled(relay_display, if is_selected { base } else { Style::default().fg(Color::DarkGray) }),
            ]);

            ListItem::new(line)
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = match app.language() {
        crate::i18n::Language::Chinese => format!("🔭 远程观察者 ({})", sessions.len()),
        crate::i18n::Language::English => format!("🔭 Remote Viewers ({})", sessions.len()),
    };
    let list = List::new(items)
        .scroll_padding(app.scroll_padding())
        .block(
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

    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);

    if tree.is_empty() {
        let empty_msg = if is_zh {
            "未找到会话。\n\n使用: agent-hand add ...\n按 'n' 创建新会话。\n按 '?' 查看帮助。"
        } else {
            "No sessions found.\n\nUse: agent-hand add ...\nPress 'n' to create.\nPress '?' for help."
        };
        let empty = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(
                if is_zh { "会话" } else { "Sessions" }
            ));

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
                                // Show viewer count and names if available
                                #[cfg(feature = "pro")]
                                {
                                    if let Some(relay) = app.relay_client(&session.id) {
                                        let vc = relay.viewer_count();
                                        let viewers = relay.viewers();
                                        let perm = &sharing.default_permission;
                                        if vc > 0 {
                                            // Add connection pulse indicator
                                            spans.push(Span::styled(
                                                connection_pulse(app.tick_count()),
                                                Style::default().fg(Color::Green),
                                            ));
                                            // Show RW controller first (if any), then RO viewers
                                            let rw_viewer = viewers.iter().find(|v| v.permission == "rw");
                                            let ro_viewers: Vec<_> = viewers.iter().filter(|v| v.permission != "rw").collect();

                                            if let Some(rw) = rw_viewer {
                                                let name = truncate_name(&rw.display_name, 12);
                                                spans.push(Span::styled(
                                                    format!("[{} {}v ctrl:", perm, vc),
                                                    Style::default().fg(Color::Green),
                                                ));
                                                spans.push(Span::styled(
                                                    format!("{}", name),
                                                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                                                ));
                                                // Show RO viewers after controller
                                                let max_ro = 2;
                                                for (vi, v) in ro_viewers.iter().take(max_ro).enumerate() {
                                                    let sep = if vi == 0 { " +" } else { "," };
                                                    let name = truncate_name(&v.display_name, 8);
                                                    spans.push(Span::styled(
                                                        format!("{}{}", sep, name),
                                                        Style::default().fg(Color::DarkGray),
                                                    ));
                                                }
                                                if ro_viewers.len() > max_ro {
                                                    spans.push(Span::styled(
                                                        format!(" +{}", ro_viewers.len() - max_ro),
                                                        Style::default().fg(Color::DarkGray),
                                                    ));
                                                }
                                            } else {
                                                spans.push(Span::styled(
                                                    format!("[{} {}v", perm, vc),
                                                    Style::default().fg(Color::Green),
                                                ));
                                                // No RW controller — show viewers normally
                                                let max_show = 3;
                                                for (vi, v) in viewers.iter().take(max_show).enumerate() {
                                                    let sep = if vi == 0 { " " } else { "," };
                                                    let name = truncate_name(&v.display_name, 10);
                                                    spans.push(Span::styled(
                                                        format!("{}{}", sep, name),
                                                        Style::default().fg(Color::DarkGray),
                                                    ));
                                                }
                                                if viewers.len() > max_show {
                                                    spans.push(Span::styled(
                                                        format!(" +{}", viewers.len() - max_show),
                                                        Style::default().fg(Color::DarkGray),
                                                    ));
                                                }
                                            }
                                            spans.push(Span::styled("]", Style::default().fg(Color::Green)));
                                        } else {
                                            spans.push(Span::styled(
                                                format!("[share: {}]", perm),
                                                Style::default().fg(Color::Green),
                                            ));
                                        }
                                    } else {
                                        spans.push(Span::styled(
                                            format!("[share: {}]", sharing.default_permission),
                                            Style::default().fg(Color::Green),
                                        ));
                                    }
                                }
                                #[cfg(not(feature = "pro"))]
                                {
                                    spans.push(Span::styled(
                                        format!("[share: {}]", sharing.default_permission),
                                        Style::default().fg(Color::Green),
                                    ));
                                }
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

    let list = List::new(items)
        .scroll_padding(app.scroll_padding())
        .block(
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
    let preview_label = match app.language() {
        crate::i18n::Language::Chinese => "预览",
        crate::i18n::Language::English => "Preview",
    };
    let title = match app.selected_item() {
        Some(TreeItem::Session { id, .. }) => app
            .session_by_id(id)
            .map(|s| format!("{preview_label} • {}", s.title))
            .unwrap_or_else(|| preview_label.to_string()),
        Some(TreeItem::Group { name, .. }) => format!("{preview_label} • {}", name),
        _ => preview_label.to_string(),
    };

    let p = Paragraph::new(app.preview())
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(p, area);
}

fn render_dialog(f: &mut Frame, area: Rect, app: &App) {
    let lang = app.language();
    let is_zh = matches!(lang, crate::i18n::Language::Chinese);

    if app.quit_confirm_dialog() {
        render_quit_confirm_dialog(f, area, is_zh);
        return;
    }

    if let Some(d) = app.new_session_dialog() {
        render_new_session_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.delete_confirm_dialog() {
        render_delete_confirm_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.delete_group_dialog() {
        render_delete_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.fork_dialog() {
        render_fork_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.create_group_dialog() {
        render_create_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.move_group_dialog() {
        render_move_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.rename_session_dialog() {
        render_rename_session_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.tag_picker_dialog() {
        render_tag_picker_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.rename_group_dialog() {
        render_rename_group_dialog(f, area, d, is_zh);
        return;
    }

    if let Some(d) = app.settings_dialog() {
        render_settings_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.control_request_dialog() {
        render_control_request_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.orphaned_rooms_dialog() {
        render_orphaned_rooms_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.pack_browser_dialog() {
        render_pack_browser_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.join_session_dialog() {
        render_join_session_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.disconnect_viewer_dialog() {
        render_disconnect_viewer_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.share_dialog() {
        render_share_dialog(f, area, d, app);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.create_relationship_dialog() {
        render_create_relationship_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.annotate_dialog() {
        render_annotate_dialog(f, area, d, is_zh);
        return;
    }

    #[cfg(feature = "pro")]
    if let Some(d) = app.new_from_context_dialog() {
        render_new_from_context_dialog(f, area, d, is_zh);
    }
}

fn render_new_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::NewSessionDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_path_active = d.field == crate::ui::NewSessionField::Path;
    let is_title_active = d.field == crate::ui::NewSessionField::Title;
    let is_group_active = d.field == crate::ui::NewSessionField::Group;

    let mut path_spans = vec![Span::raw(if is_zh { "路径:   " } else { "Path:   " })];
    path_spans.extend(render_text_input(&d.path, is_path_active, base_style));

    let mut title_spans = vec![Span::raw(if is_zh { "标题:   " } else { "Title:  " })];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw(if is_zh { "分组:   " } else { "Group:  " })];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "新建会话" } else { "New Session" },
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
                if is_zh { "(未找到; 将创建目录)" } else { "(not found; will create directory)" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    if d.path_suggestions_visible && !d.path_suggestions.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled(if is_zh { "建议:" } else { "Suggestions:" }, Style::default().fg(Color::DarkGray)),
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
            if is_zh { "分组 (↑/↓ 选择):" } else { "Groups (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )));

        if d.group_matches.is_empty() {
            lines.push(Line::from(Span::styled(
                if is_zh { "(无匹配)" } else { "(no matches)" },
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
                let label = if g.is_empty() { if is_zh { "(无)" } else { "(none)" } } else { g.as_str() };
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
        if is_zh { "Tab: 补全路径 • ↑↓: 选择 • 回车: 下一个/提交 • Esc/Ctrl+C: 取消" } else { "Tab: complete path • ↑↓: pick • Enter: next/submit • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "新建" } else { "New" }));

    f.render_widget(p, popup_area);
}

fn render_fork_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ForkDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::ForkField::Title;
    let is_group_active = d.field == crate::ui::ForkField::Group;

    let mut title_spans = vec![Span::raw(if is_zh { "标题: " } else { "Title: " })];
    title_spans.extend(render_text_input(&d.title, is_title_active, base_style));

    let mut group_spans = vec![Span::raw(if is_zh { "分组: " } else { "Group: " })];
    group_spans.extend(render_text_input(
        &d.group_path,
        is_group_active,
        base_style,
    ));

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "复制会话" } else { "Fork Session" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(group_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "Tab: 切换字段 • 回车: 下一个/提交 • Esc/Ctrl+C: 取消" } else { "Tab: switch field • Enter: next/submit • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "复制" } else { "Fork" }));

    f.render_widget(p, popup_area);
}

fn render_create_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::CreateGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw(if is_zh { "名称:   " } else { "Name:   " })];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "创建分组" } else { "Create Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "已有分组 (↑/↓ 选择):" } else { "Existing (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            if is_zh { "(无匹配)" } else { "(no matches)" },
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
        if is_zh { "输入过滤/命名 • 回车: 创建 • Esc/Ctrl+C: 取消" } else { "Type to filter/name • Enter: create • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

fn render_move_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::MoveGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(75, 60, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut input_spans = vec![Span::raw(if is_zh { "过滤: " } else { "Filter: " })];
    input_spans.extend(render_text_input(&d.input, true, base_style));

    let mut lines = vec![
        Line::from(Span::styled(
            if is_zh { "移动会话到分组" } else { "Move Session to Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "标题:  " } else { "Title:  " }),
            Span::styled(
                d.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(input_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "分组 (↑/↓ 选择):" } else { "Groups (↑/↓ to select):" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if d.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            if is_zh { "(无匹配)" } else { "(no matches)" },
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
            let label = if g.is_empty() { if is_zh { "(无)" } else { "(none)" } } else { g.as_str() };
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
        if is_zh { "输入过滤 • 回车: 应用 • Esc/Ctrl+C: 取消" } else { "Type to filter • Enter: apply • Esc/Ctrl+C: cancel" },
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

fn render_tag_picker_dialog(f: &mut Frame, area: Rect, d: &crate::ui::TagPickerDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(popup_area);

    if d.tags.is_empty() {
        let empty = Paragraph::new(
            if is_zh { "(未找到标签)\n\n提示: 先编辑会话标签 (r), 然后在此处复用。" } else { "(no tags found)\n\nTip: edit a session label first (r), then reuse it here." },
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "标签" } else { "Tag" }));
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

        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(if is_zh { "标签" } else { "Tag" }));
        let mut state = ListState::default().with_selected(Some(d.selected));
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    let hint = Paragraph::new(if is_zh { "↑/↓: 选择 • 回车: 应用 • Esc: 取消" } else { "↑/↓: select • Enter: apply • Esc: cancel" })
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(hint, chunks[1]);
}

fn render_rename_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameSessionDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 40, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let is_title_active = d.field == crate::ui::SessionEditField::Title;
    let is_label_active = d.field == crate::ui::SessionEditField::Label;

    let mut title_spans = vec![Span::raw(if is_zh { "标题:  " } else { "Title:  " })];
    title_spans.extend(render_text_input(&d.new_title, is_title_active, base_style));

    let mut label_spans = vec![Span::raw(if is_zh { "标签:  " } else { "Label:  " })];
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
            if is_zh { "编辑会话" } else { "Edit Session" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(title_spans),
        Line::from(label_spans),
        Line::from(vec![
            Span::raw(if is_zh { "颜色:  " } else { "Color:  " }),
            Span::styled(
                format!("{color_name}"),
                color_style.fg(color_fg).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(if is_zh { ":下一字段  " } else { ":next field  " }),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(if is_zh { ":下一个/应用  " } else { ":next/apply  " }),
            Span::styled("←/→", Style::default().fg(Color::Yellow)),
            Span::raw(if is_zh { ":颜色  " } else { ":color  " }),
            Span::styled("Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(if is_zh { ":取消" } else { ":cancel" }),
        ]),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "会话" } else { "Session" }));

    f.render_widget(p, popup_area);
}

fn render_rename_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::RenameGroupDialog, is_zh: bool) {
    let popup_area = centered_rect(70, 35, area);
    f.render_widget(Clear, popup_area);

    let base_style = Style::default();
    let mut new_path_spans = vec![Span::raw(if is_zh { "到:    " } else { "To:    " })];
    new_path_spans.extend(render_text_input(&d.new_path, true, base_style));

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "重命名分组" } else { "Rename Group" },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "从:    " } else { "From:  " }),
            Span::styled(d.old_path.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(new_path_spans),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "回车: 应用 • Esc/Ctrl+C: 取消" } else { "Enter: apply • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "分组" } else { "Group" }));

    f.render_widget(p, popup_area);
}

fn render_quit_confirm_dialog(f: &mut Frame, area: Rect, is_zh: bool) {
    let popup_area = centered_rect(40, 20, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "退出 Agent Hand？" } else { "Quit Agent Hand?" },
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(if is_zh { "再按 q 退出。" } else { "Press q again to quit." }),
        Line::from(if is_zh { "按其他键取消。" } else { "Any other key to cancel." }),
    ];

    let p = Paragraph::new(lines)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(
            if is_zh { "确认退出" } else { "Confirm Quit" }
        ));

    f.render_widget(p, popup_area);
}

fn render_delete_confirm_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteConfirmDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::from(Span::styled(
            if is_zh { "删除会话？" } else { "Delete session?" },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "标题: " } else { "Title: " }),
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
            Span::raw(if is_zh { "终止 tmux 会话: " } else { "Kill tmux session: " }),
            Span::styled(
                if is_zh { if d.kill_tmux { "是" } else { "否" } } else { if d.kill_tmux { "YES" } else { "NO" } },
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
            Span::raw(if is_zh { "  (按 't' 切换)" } else { "  (press 't' to toggle)" }),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "y/回车: 确认 • n/Esc/Ctrl+C: 取消" } else { "y/Enter: confirm • n/Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "确认" } else { "Confirm" }));

    f.render_widget(p, popup_area);
}

fn render_delete_group_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DeleteGroupDialog, is_zh: bool) {
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
            if is_zh { "删除分组？" } else { "Delete group?" },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "分组: " } else { "Group: " }),
            Span::styled(
                d.group_path.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(if is_zh { "会话数: " } else { "Sessions: " }),
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
            Span::styled(if is_zh { "仅删除分组 (保留会话)" } else { "Delete group only (keep sessions)" }, opt1_style),
        ]),
        Line::from(vec![
            Span::styled("2", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(if is_zh { "取消" } else { "Cancel" }, opt2_style),
        ]),
        Line::from(vec![
            Span::styled("3", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(if is_zh { "删除分组和会话" } else { "Delete group + sessions" }, opt3_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "1/2/3 或 ↑/↓ • 回车: 确认 • Esc/Ctrl+C: 取消" } else { "1/2/3 or ↑/↓ • Enter: confirm • Esc/Ctrl+C: cancel" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "确认" } else { "Confirm" }));

    f.render_widget(p, popup_area);
}

fn render_search_popup(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let popup_area = centered_rect(80, 60, area);
    f.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        if is_zh { "搜索" } else { "Search" },
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::raw(format!(
        "{}: {}",
        if is_zh { "查询" } else { "Query" },
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
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "搜索" } else { "Search" }));

    f.render_widget(p, popup_area);
}

fn render_settings_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::SettingsDialog,
    is_zh: bool,
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
        if is_zh { " 设置" } else { " Settings" },
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
        let tab_label = if is_zh {
            match tab {
                SettingsTab::AI => "AI",
                SettingsTab::Sharing => "共享",
                #[cfg(feature = "pro")]
                SettingsTab::Notification => "音效",
                SettingsTab::General => "通用",
            }
        } else {
            tab.label()
        };
        let label = format!(" {} ", tab_label);
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
        let field_label = if is_zh {
            match field {
                SettingsField::AiProvider => "AI 提供商",
                SettingsField::AiApiKey => "API 密钥",
                SettingsField::AiModel => "模型",
                SettingsField::AiBaseUrl => "Base URL",
                SettingsField::AiSummaryLines => "摘要行数",
                SettingsField::AiTest => "测试连接",
                SettingsField::RelayServerUrl => "中继服务器",
                SettingsField::TmateHost => "tmate 主机",
                SettingsField::TmatePort => "tmate 端口",
                SettingsField::DefaultPermission => "默认权限",
                SettingsField::AutoExpire => "自动过期 (分)",
                #[cfg(feature = "pro")]
                SettingsField::NotifHookStatus => "Hook 状态",
                #[cfg(feature = "pro")]
                SettingsField::NotifAutoRegister => "自动注册",
                #[cfg(feature = "pro")]
                SettingsField::NotifEnabled => "启用",
                #[cfg(feature = "pro")]
                SettingsField::NotifSoundPack => "音效包",
                #[cfg(feature = "pro")]
                SettingsField::NotifOnComplete => "完成时",
                #[cfg(feature = "pro")]
                SettingsField::NotifOnInput => "输入时",
                #[cfg(feature = "pro")]
                SettingsField::NotifOnError => "错误时",
                #[cfg(feature = "pro")]
                SettingsField::NotifVolume => "音量",
                #[cfg(feature = "pro")]
                SettingsField::NotifTestSound => "测试音效",
                #[cfg(feature = "pro")]
                SettingsField::NotifPackLink => "安装音效包",
                SettingsField::AnalyticsEnabled => "分析",
                SettingsField::MouseCapture => "鼠标捕获",
                SettingsField::JumpLines => "跳转行数",
                SettingsField::ScrollPadding => "滚动边距",
                SettingsField::ReadyTtl => "就绪 TTL (分)",
                SettingsField::Language => "语言",
            }
        } else {
            field.label()
        };
        let label = format!("  {:<16}", field_label);
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
            SettingsField::MouseCapture => {
                let is_editing_this = d.editing && is_active;
                let labels = ["Auto", "On", "Off"];
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    for (i, label) in labels.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(" "));
                        }
                        let style = if d.mouse_capture_mode == i as u8 { sel } else { unsel };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                } else {
                    let val = labels[d.mouse_capture_mode as usize % 3];
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // ── Notification tab fields (Pro) — Hook Integration ──
            #[cfg(feature = "pro")]
            SettingsField::NotifHookStatus => {
                // Section header
                lines.push(Line::from(spans));
                // Show each tool's status
                for (i, info) in d.hook_tools.iter().enumerate() {
                    let is_sel = d.editing && is_active && i == d.hook_selected_tool;
                    let (sym, sym_color) = match info.status {
                        agent_hooks::ToolStatus::HooksRegistered => ("\u{2713}", Color::Green),
                        agent_hooks::ToolStatus::Detected => ("\u{25cf}", Color::Yellow),
                        agent_hooks::ToolStatus::NotInstalled => ("\u{2717}", Color::DarkGray),
                    };
                    let name_style = if is_sel {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else if info.status == agent_hooks::ToolStatus::NotInstalled {
                        dim_style
                    } else {
                        base_style
                    };
                    let mut row: Vec<Span<'static>> = vec![
                        Span::raw("    "),
                        Span::styled(format!("{sym} "), Style::default().fg(sym_color)),
                        Span::styled(format!("{:<16}", info.display_name), name_style),
                        Span::styled(info.status.label().to_string(), Style::default().fg(sym_color)),
                    ];
                    if is_sel && info.status == agent_hooks::ToolStatus::Detected {
                        row.push(Span::styled("  (Enter: install)", dim_style));
                    } else if is_sel && info.status == agent_hooks::ToolStatus::HooksRegistered {
                        row.push(Span::styled("  (Enter: uninstall)", dim_style));
                    }
                    lines.push(Line::from(row));
                }
                if !d.editing && is_active {
                    lines.push(Line::from(Span::styled(
                        "                  (Enter to manage)",
                        dim_style,
                    )));
                }
                continue;
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifAutoRegister => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.hook_auto_register { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.hook_auto_register { sel } else { unsel }));
                } else {
                    let val = if d.hook_auto_register { "On" } else { "Off" };
                    spans.push(Span::styled(format!("\u{25b8} {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // ── Notification tab fields (Pro) — Sound section ──
            #[cfg(feature = "pro")]
            SettingsField::NotifEnabled => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_enabled { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_enabled { sel } else { unsel }));
                } else {
                    let val = if d.notif_enabled { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifSoundPack => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    // Expanded chip selector: show all pack names
                    let max_width = popup_area.width.saturating_sub(22) as usize;
                    let mut row_width = 0usize;
                    let mut first_in_row = true;
                    let mut overflow_lines: Vec<Vec<Span<'static>>> = Vec::new();
                    let current_spans = &mut spans;

                    if d.notif_pack_names.is_empty() {
                        current_spans.push(Span::styled(
                            "(no packs installed)",
                            dim_style,
                        ));
                    } else {
                        for (i, name) in d.notif_pack_names.iter().enumerate() {
                            let chip = format!(" {} ", name);
                            let chip_len = chip.len() + 1;
                            if !first_in_row && row_width + chip_len > max_width {
                                overflow_lines.push(Vec::new());
                                row_width = 0;
                                first_in_row = true;
                            }

                            let style = if i == d.notif_pack_idx {
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
                    let val = format!("▸ {}", d.pack_display());
                    spans.push(Span::styled(val, if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifOnComplete => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_complete { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_complete { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_complete { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifOnInput => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_input { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_input { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_input { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifOnError => {
                let is_editing_this = d.editing && is_active;
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    spans.push(Span::styled(" Off ", if !d.notif_on_error { sel } else { unsel }));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(" On ", if d.notif_on_error { sel } else { unsel }));
                } else {
                    let val = if d.notif_on_error { "On" } else { "Off" };
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifVolume => {
                if d.editing && is_active {
                    spans.extend(render_text_input(&d.notif_volume, true, base_style));
                    spans.push(Span::styled("%", base_style));
                } else {
                    let t = d.notif_volume.text();
                    spans.push(Span::styled(
                        format!("{t}%"),
                        if is_active { active_style } else { base_style },
                    ));
                }
            }
            #[cfg(feature = "pro")]
            SettingsField::NotifTestSound => {
                if let Some(status) = &d.notif_test_status {
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
            #[cfg(feature = "pro")]
            SettingsField::NotifPackLink => {
                if is_active {
                    spans.push(Span::styled(
                        "Enter to browse",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        "Browse & install",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            SettingsField::Language => {
                let is_editing_this = d.editing && is_active;
                let labels = ["English", "中文"];
                if is_editing_this {
                    let sel = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    let unsel = Style::default().fg(Color::DarkGray);
                    for (i, label) in labels.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(" "));
                        }
                        let style = if d.language_idx == i { sel } else { unsel };
                        spans.push(Span::styled(format!(" {label} "), style));
                    }
                } else {
                    let val = labels[d.language_idx % 2];
                    spans.push(Span::styled(format!("▸ {val}"), if is_active { active_style } else { base_style }));
                    if is_active {
                        spans.push(Span::styled("  (Enter to select)", dim_style));
                    }
                }
            }
            // Text input fields: relay_url, auto_expire, jump_lines, scroll_padding, ready_ttl
            _ => {
                let input = match field {
                    SettingsField::RelayServerUrl => &d.relay_url,
                    SettingsField::AutoExpire => &d.auto_expire,
                    SettingsField::JumpLines => &d.jump_lines,
                    SettingsField::ScrollPadding => &d.scroll_padding,
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
            if is_zh { "  ● 未保存的更改" } else { "  ● Unsaved changes" },
            Style::default().fg(Color::Yellow),
        )));
    }

    // Key hints
    lines.push(Line::from(""));
    let hint_style = Style::default().fg(Color::DarkGray);
    if d.editing {
        let is_selector = d.field.is_selector();
        if is_selector {
            lines.push(Line::from(Span::styled(
                if is_zh { "  ←/→:选择  回车/Esc:完成" } else { "  ←/→:choose  Enter/Esc:done" },
                hint_style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                if is_zh { "  输入编辑  回车/Esc:完成" } else { "  type to edit  Enter/Esc:done" },
                hint_style,
            )));
        }
    } else {
        if is_zh {
            lines.push(Line::from(vec![
                Span::styled("  ←/→:切换标签  ", hint_style),
                Span::styled("j/k:选择字段  ", hint_style),
                Span::styled("回车:编辑  ", hint_style),
                Span::styled("Ctrl+S:保存  ", hint_style),
                Span::styled("Esc:关闭", hint_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  ←/→:tab  ", hint_style),
                Span::styled("j/k:field  ", hint_style),
                Span::styled("Enter:edit  ", hint_style),
                Span::styled("Ctrl+S:save  ", hint_style),
                Span::styled("Esc:close", hint_style),
            ]));
        }
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(if is_zh { "设置" } else { "Settings" }));

    f.render_widget(p, popup_area);
}

#[cfg(feature = "pro")]
fn truncate_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let end = name.char_indices().nth(max).map(|(i, _)| i).unwrap_or(name.len());
        format!("{}..", &name[..end])
    }
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
fn render_onboarding_welcome(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
    use crate::i18n::Language;

    let modal_area = centered_rect(70, 60, area);
    f.render_widget(Clear, modal_area);

    let (title_str, welcome_title, desc, features_title, f1, f2, f3, f4,
         start_title, s1, s2, s3, s4, continue_str) = match lang {
        Language::Chinese => (
            " 欢迎 ",
            "欢迎使用 Agent Deck！",
            "Agent Deck 帮助您高效管理多个 AI 智能体会话。",
            "主要功能：",
            "  • 分组管理会话",
            "  • 启动、停止和连接会话",
            "  • 实时协作（Pro）",
            "  • 会话分享和观察者模式（Pro）",
            "快速开始：",
            "  • 按 's' 创建新会话",
            "  • 使用 ↑/↓ 或 j/k 导航",
            "  • 按 Enter 连接到会话",
            "  • 随时按 '?' 查看帮助",
            "按 Enter 继续...",
        ),
        Language::English => (
            " Welcome ",
            "Welcome to Agent Deck!",
            "Agent Deck helps you manage multiple AI agent sessions efficiently.",
            "Key Features:",
            "  • Organize sessions in groups",
            "  • Start, stop, and attach to sessions",
            "  • Real-time collaboration (Pro)",
            "  • Session sharing and viewer mode (Pro)",
            "Quick Start:",
            "  • Press 's' to start a new session",
            "  • Use ↑/↓ or j/k to navigate",
            "  • Press Enter to attach to a session",
            "  • Press '?' anytime for help",
            "Press Enter to continue...",
        ),
    };

    let welcome_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            welcome_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(desc),
        Line::from(""),
        Line::from(Span::styled(
            features_title,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(f1),
        Line::from(f2),
        Line::from(f3),
        Line::from(f4),
        Line::from(""),
        Line::from(Span::styled(
            start_title,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(s1),
        Line::from(s2),
        Line::from(s3),
        Line::from(s4),
        Line::from(""),
        Line::from(Span::styled(
            continue_str,
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(welcome_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(title_str),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, modal_area);
}

fn render_help_modal(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
    use crate::i18n::Language;

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

    let hint = |text: &str| -> Line<'static> {
        Line::from(Span::styled(
            format!("  {text}"),
            Style::default().fg(Color::DarkGray),
        ))
    };

    let is_zh = matches!(lang, Language::Chinese);

    let help_text: Vec<Line<'static>> = vec![
        Line::from(""),
        section(if is_zh { "导航" } else { "Navigation" }),
        hint(if is_zh { "在树形视图中浏览会话和分组" } else { "Browse sessions and groups in the tree view" }),
        key("↑/k", if is_zh { "向上移动" } else { "Move up" }),
        key("↓/j", if is_zh { "向下移动" } else { "Move down" }),
        key("←/→", if is_zh { "展开/折叠分组" } else { "Expand/collapse group" }),
        key("/", if is_zh { "按名称搜索会话" } else { "Search sessions by name" }),
        key("Tab", if is_zh { "切换面板焦点：活跃→观察→树 (Pro)" } else { "Cycle focus: Active → Viewer → Tree (Pro)" }),
        Line::from(""),
        section(if is_zh { "会话操作" } else { "Session Actions" }),
        hint(if is_zh { "在树形视图中管理单个会话" } else { "Manage individual sessions from the tree view" }),
        key("Enter", if is_zh { "连接到所选会话的终端" } else { "Attach to the selected session's terminal" }),
        key("s", if is_zh { "启动已停止的会话" } else { "Start a stopped session" }),
        key("x", if is_zh { "停止正在运行的会话" } else { "Stop a running session" }),
        key("r", if is_zh { "编辑会话名称或配置" } else { "Edit session name or configuration" }),
        key("R", if is_zh { "重启：先停止再启动会话" } else { "Restart: stop then start a session" }),
        key("m", if is_zh { "将会话移动到其他分组" } else { "Move session to a different group" }),
        key("f", if is_zh { "复制：创建会话副本" } else { "Fork: create a copy of the session" }),
        key("d", if is_zh { "永久删除会话" } else { "Delete session permanently" }),
        key("b", if is_zh { "提升：将会话置顶到活跃面板" } else { "Boost: bring session to active panel" }),
        #[cfg(feature = "max")]
        key("A", if is_zh { "AI 总结会话输出 (Max)" } else { "AI summary of session output (Max)" }),
        Line::from(""),
        section(if is_zh { "分组操作" } else { "Group Actions" }),
        hint(if is_zh { "将会话整理到可折叠的分组中" } else { "Organize sessions into collapsible groups" }),
        key("Enter", if is_zh { "展开/折叠分组" } else { "Toggle group expand/collapse" }),
        key("r", if is_zh { "重命名分组" } else { "Rename group" }),
        key("d", if is_zh { "删除分组（会话保留）" } else { "Delete group (sessions are unlinked)" }),
        Line::from(""),
        section(if is_zh { "全局" } else { "Global" }),
        hint(if is_zh { "在任何界面均可使用" } else { "Available from any screen" }),
        key("n", if is_zh { "创建新会话" } else { "Create a new session" }),
        key("g", if is_zh { "创建新分组" } else { "Create a new group" }),
        key("p", if is_zh { "预览最近的会话快照" } else { "Preview latest session snapshot" }),
        key("Ctrl+r", if is_zh { "强制刷新所有会话状态" } else { "Force refresh all session statuses" }),
        key("Ctrl+e", if is_zh { "查看会话关系图" } else { "View session relationships graph" }),
        key("Shift+S", if is_zh { "通过中继分享会话 (Pro)" } else { "Share session via relay (Pro)" }),
        key("Shift+J", if is_zh { "通过 URL 加入共享会话 (Pro)" } else { "Join a shared session by URL (Pro)" }),
        key(",", if is_zh { "打开设置" } else { "Open settings" }),
        key("?", if is_zh { "切换帮助界面" } else { "Toggle this help screen" }),
        key("q", if is_zh { "退出 Agent Deck" } else { "Quit Agent Deck" }),
        Line::from(""),
        section(if is_zh { "观察者会话面板 (Pro)" } else { "Viewer Sessions Panel (Pro)" }),
        hint(if is_zh { "管理已连接的共享会话" } else { "Manage shared sessions you've connected to" }),
        key("↑/↓", if is_zh { "浏览观察者会话列表" } else { "Navigate viewer sessions list" }),
        key("Enter", if is_zh { "切换到或重新连接观察者会话" } else { "Switch to or reconnect a viewer session" }),
        key("d", if is_zh { "打开断开连接对话框" } else { "Open disconnect dialog for session" }),
        key("Ctrl+Q", if is_zh { "从观察模式返回仪表盘" } else { "Return to Dashboard from viewer mode" }),
        Line::from(""),
        section(if is_zh { "观察者模式 (Pro)" } else { "Viewer Mode (Pro)" }),
        hint(if is_zh { "观看或控制共享的远程会话" } else { "Watch or control a shared remote session" }),
        key("Up/Down", if is_zh { "滚动(只读) / 发送输入(读写)" } else { "Scroll (RO) / Send input (RW)" }),
        key("Shift+Up/Dn", if is_zh { "读写模式下滚动" } else { "Scroll while in RW mode" }),
        key("PgUp/PgDn", if is_zh { "翻页(只读) / 发送输入(读写)" } else { "Page scroll (RO) / Send input (RW)" }),
        key("Home/End", if is_zh { "跳到顶部 / 跟随最新输出" } else { "Jump to top / Follow latest output" }),
        key("F1-F12", if is_zh { "读写模式下转发到会话" } else { "Forwarded to session in RW mode" }),
        key("r", if is_zh { "请求读写控制权" } else { "Request read-write control" }),
        key("Esc", if is_zh { "读写：释放控制 / 只读：断开" } else { "RW: relinquish control / RO: disconnect" }),
        key("q", if is_zh { "断开观察者会话连接(只读)" } else { "Disconnect from viewer session (RO)" }),
        key("Ctrl+V", if is_zh { "在加入对话框粘贴 URL" } else { "Paste URL in join dialog" }),
        Line::from(""),
        section(if is_zh { "状态指示器" } else { "Status Indicators" }),
        hint(if is_zh { "树形视图中的会话状态图标" } else { "Session status icons in the tree view" }),
        Line::from(vec![
            Span::styled("  !  ", Style::default().fg(Color::Blue)),
            Span::raw(if is_zh { "等待中" } else { "WAITING" }),
            Span::raw("    "),
            Span::styled("✓  ", Style::default().fg(Color::Cyan)),
            Span::raw(if is_zh { "就绪" } else { "READY" }),
            Span::raw("     "),
            Span::styled("●  ", Style::default().fg(Color::Yellow)),
            Span::raw(if is_zh { "运行中" } else { "RUNNING" }),
        ]),
        Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::raw(if is_zh { "空闲" } else { "IDLE" }),
            Span::raw("       "),
            Span::styled("✕  ", Style::default().fg(Color::Red)),
            Span::raw(if is_zh { "错误" } else { "ERROR" }),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if is_zh { "          按 ? 或 Esc 关闭" } else { "          Press ? or Esc to close" },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help_title = if is_zh { " ⌨ 命令与快捷键 " } else { " ⌨ Commands & Shortcuts " };
    let help = Paragraph::new(help_text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    help_title,
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
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    match app.selected_item() {
        Some(TreeItem::Group { .. }) => {
            spans.push(Span::styled("Enter", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":切换  " } else { ":toggle  " }));
            spans.push(Span::styled("r", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(if is_zh { ":重命名  " } else { ":rename  " }));
            spans.push(Span::styled("d", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":删除  " } else { ":del  " }));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新分组  " } else { ":group+  " }));
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新建  " } else { ":new  " }));
        }
        Some(TreeItem::Session { .. }) => {
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新建  " } else { ":new  " }));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新分组  " } else { ":group+  " }));
            spans.push(Span::styled("r", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(if is_zh { ":重命名  " } else { ":rename  " }));
            spans.push(Span::styled("R", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(if is_zh { ":重启  " } else { ":restart  " }));
            spans.push(Span::styled("d", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":删除  " } else { ":del  " }));
            spans.push(Span::styled("f", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":复制  " } else { ":fork  " }));
            spans.push(Span::styled("m", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":移动  " } else { ":move  " }));
            spans.push(Span::styled("b", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":置顶  " } else { ":boost  " }));
            #[cfg(feature = "max")]
            {
                spans.push(Span::styled("A", Style::default().fg(Color::Magenta)));
                spans.push(Span::raw(":AI  "));
            }
        }
        _ => {
            spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新建  " } else { ":new  " }));
            spans.push(Span::styled("g", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh { ":新分组  " } else { ":group+  " }));
        }
    }
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);

    // Viewer mode has its own presence bar — skip the normal status bar
    #[cfg(feature = "pro")]
    if app.state() == crate::ui::AppState::ViewerMode {
        let mut spans = vec![
            Span::raw("  "),
            Span::styled("?", Style::default().fg(Color::Magenta)),
            Span::raw(if is_zh { ":帮助  " } else { ":help  " }),
        ];
        // Show hosting indicator if user has active shared sessions
        let hosting = app.hosting_session_count();
        if hosting > 0 {
            spans.push(Span::styled("|  ", Style::default().fg(Color::DarkGray)));
            let hosting_msg = if is_zh {
                format!("正在共享 {} 个会话", hosting)
            } else {
                format!("Hosting {} session{}", hosting, if hosting == 1 { "" } else { "s" })
            };
            spans.push(Span::styled(hosting_msg, Style::default().fg(Color::Yellow)));
            spans.push(Span::raw("  "));
        }
        let bar = Paragraph::new(Line::from(spans))
            .style(Style::default().bg(Color::Rgb(20, 20, 35)));
        f.render_widget(bar, area);
        return;
    }

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

    // Mouse capture hint — show platform-aware copy instructions
    if app.mouse_captured() {
        let select_hint = if cfg!(target_os = "macos") {
            "Opt+Drag"
        } else {
            "Shift+Drag"
        };
        spans.push(Span::styled(
            select_hint,
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            if is_zh { ":选择文本  " } else { ":select text  " },
            Style::default().fg(Color::DarkGray),
        ));
    }

    #[cfg(feature = "pro")]
    if app.state() == crate::ui::AppState::Relationships {
        spans.push(Span::styled("n", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(if is_zh { ":新建  " } else { ":new  " }));
        spans.push(Span::styled("d", Style::default().fg(Color::Red)));
        spans.push(Span::raw(if is_zh { ":删除  " } else { ":del  " }));
        spans.push(Span::styled("c", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(if is_zh { ":捕获  " } else { ":capture  " }));
        spans.push(Span::styled("a", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(if is_zh { ":注释  " } else { ":annotate  " }));
        spans.push(Span::styled("^N", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(if is_zh { ":从上下文  " } else { ":from-ctx  " }));
        spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(if is_zh { ":返回" } else { ":back" }));
    } else {
        render_item_hints(&mut spans, app);
    }
    #[cfg(not(feature = "pro"))]
    render_item_hints(&mut spans, app);

    spans.push(Span::styled("/", Style::default().fg(Color::Cyan)));
    spans.push(Span::raw(if is_zh { ":搜索  " } else { ":search  " }));
    #[cfg(feature = "pro")]
    {
        spans.push(Span::styled("^E", Style::default().fg(Color::Cyan)));
        spans.push(Span::raw(if is_zh { ":关系  " } else { ":rels  " }));
    }
    spans.push(Span::styled("p", Style::default().fg(Color::Cyan)));
    spans.push(Span::raw(if is_zh { ":预览  " } else { ":preview  " }));
    spans.push(Span::styled("?", Style::default().fg(Color::Magenta)));
    spans.push(Span::raw(if is_zh { ":帮助  " } else { ":help  " }));
    spans.push(Span::styled("q", Style::default().fg(Color::Red)));
    spans.push(Span::raw(if is_zh { ":退出" } else { ":quit" }));

    if app.state() == crate::ui::AppState::Search {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            if is_zh { "搜索: " } else { "Search: " },
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
            spans.push(Span::raw(if is_zh { ":活跃面板" } else { ":active-panel" }));
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
        spans.push(Span::styled(
            if is_zh { "未登录" } else { "not logged in" },
            Style::default().fg(Color::DarkGray),
        ));
    }

    let status_line = Line::from(spans);

    let status = Paragraph::new(status_line).block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

/// Render the Relationships view (Premium)
#[cfg(feature = "pro")]
fn render_relationships(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let relationships = app.relationships();
    let selected = app.selected_relationship_index();

    // Left panel: relationship list with selection
    let items: Vec<ListItem> = if relationships.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            if is_zh { "  暂无关系。按 'n' 新建。" } else { "  No relationships yet. Press 'n' to create one." },
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
                            Span::styled(if is_zh { " ✓ 就绪" } else { " ✓ ready" }, Style::default().fg(Color::Green))
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
            .title(if is_zh { format!("关系 ({})", relationships.len()) } else { format!("Relationships ({})", relationships.len()) }),
    );
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Right panel: context preview
    let preview_text = if relationships.is_empty() {
        if is_zh {
            "选择一个关系以查看上下文。\n\n\
             Ctrl+E: 返回会话\n\
             n: 新建关系\n\
             d: 删除关系\n\
             c: 捕获上下文\n\
             a: 标注"
                .to_string()
        } else {
            "Select a relationship to see context.\n\n\
             Ctrl+E: back to sessions\n\
             n: new relationship\n\
             d: delete relationship\n\
             c: capture context\n\
             a: annotate"
                .to_string()
        }
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
                if is_zh {
                    "\n✓ 依赖已满足 — 源会话空闲。\n  输出可能已准备好。\n".to_string()
                } else {
                    "\n✓ Dependency satisfied — source session is idle.\n  Output may be ready for consumption.\n".to_string()
                }
            } else {
                if is_zh {
                    "\n⏳ 依赖等待中 — 源会话仍在活跃。\n".to_string()
                } else {
                    "\n⏳ Dependency pending — source session still active.\n".to_string()
                }
            }
        } else {
            String::new()
        };

        let snap_count = app.snapshot_count(&rel.id);
        let snap_info = if snap_count > 0 {
            if is_zh { format!("\n快照: {}\n", snap_count) } else { format!("\nSnapshots: {}\n", snap_count) }
        } else {
            if is_zh { "\n尚未捕获快照。\n".to_string() } else { "\nNo snapshots captured yet.\n".to_string() }
        };

        if is_zh {
            format!(
                "=== {} ===\n\
                 类型: {}\n\
                 {}\n\
                 会话 A: \"{}\"\n\
                 状态: {}\n\n\
                 会话 B: \"{}\"\n\
                 状态: {}\n\
                 {}{}\n\
                 [c] 捕获  [a] 标注  [Ctrl+N] 从上下文新建\n\
                 [d] 删除   [Esc] 返回",
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
        }
    } else {
        if is_zh { "未选择关系。".to_string() } else { "No relationship selected.".to_string() }
    };

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if is_zh { "上下文预览" } else { "Context Preview" }),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[1]);
}

#[cfg(feature = "pro")]
fn render_pack_browser_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::dialogs::PackBrowserDialog,
    is_zh: bool,
) {
    use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(if is_zh { " 安装音效包 " } else { " Install Sound Packs " })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Layout: status bar at top, pack list, hints at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // status
            Constraint::Min(4),   // pack list
            Constraint::Length(2), // hints
        ])
        .split(inner);

    // Status bar
    let status_style = if d.status.contains("Failed") || d.status.contains("failed") {
        Style::default().fg(Color::Red)
    } else if d.status.contains("Installed") || d.status.contains("Done") {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let status = Paragraph::new(format!(" {}", d.status)).style(status_style);
    f.render_widget(status, chunks[0]);

    // Pack list
    if d.loading {
        let loading = Paragraph::new(if is_zh { "  加载中..." } else { "  Loading..." })
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(loading, chunks[1]);
    } else if d.packs.is_empty() {
        let empty = Paragraph::new(if is_zh { "  未找到音效包" } else { "  No packs found" })
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, chunks[1]);
    } else {
        let visible_height = chunks[1].height as usize;
        // Scroll to keep selected visible
        let scroll_offset = if d.selected >= visible_height {
            d.selected - visible_height + 1
        } else {
            0
        };

        let items: Vec<ListItem> = d
            .packs
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(i, pack)| {
                let marker = if pack.installed { if is_zh { " [已安装]" } else { " [installed]" } } else { "" };
                let text = format!("  {}{}", pack.name, marker);
                let style = if i == d.selected {
                    if pack.installed {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    }
                } else if pack.installed {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let list = List::new(items);
        f.render_widget(list, chunks[1]);
    }

    // Hints
    let hint = if is_zh {
        if d.installing {
            " 安装中... 请稍候".to_string()
        } else {
            " 回车: 安装  |  j/k: 浏览  |  Esc: 关闭\n 来源: github.com/PeonPing/og-packs".to_string()
        }
    } else {
        if d.installing {
            " Installing... please wait".to_string()
        } else {
            " Enter: Install  |  j/k: Navigate  |  Esc: Close\n Source: github.com/PeonPing/og-packs".to_string()
        }
    };
    let hints = Paragraph::new(hint)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hints, chunks[2]);
}

#[cfg(feature = "pro")]
fn render_join_session_dialog(f: &mut Frame, area: Rect, d: &crate::ui::JoinSessionDialog, is_zh: bool) {
    let popup_area = centered_rect(65, 35, area);
    f.render_widget(Clear, popup_area);

    let border_color = if d.connecting {
        Color::Yellow
    } else if d.status.as_ref().is_some_and(|s| s.contains("failed") || s.contains("Invalid")) {
        Color::Red
    } else {
        Color::Cyan
    };

    let block = Block::default()
        .title(if is_zh { " 加入共享会话 " } else { " Join Shared Session " })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Rgb(20, 20, 35)));
    f.render_widget(block, popup_area);

    let inner = popup_area.inner(ratatui::layout::Margin { horizontal: 2, vertical: 1 });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // identity or spacer
            Constraint::Length(1), // label
            Constraint::Length(1), // input
            Constraint::Length(1), // validation hint
            Constraint::Length(2), // status (allow 2 lines for long errors)
            Constraint::Length(1), // spacer
            Constraint::Length(1), // actions hint
        ])
        .split(inner);

    // Show identity (logged in or anonymous)
    if let Some(ref identity) = d.viewer_identity {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(if is_zh { "加入身份: " } else { "Joining as: " }, Style::default().fg(Color::DarkGray)),
                Span::styled(identity.as_str(), Style::default().fg(Color::Cyan)),
            ])),
            chunks[0],
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(if is_zh { "加入身份: " } else { "Joining as: " }, Style::default().fg(Color::DarkGray)),
                Span::styled(if is_zh { "匿名" } else { "anonymous" }, Style::default().fg(Color::Yellow)),
                Span::styled(if is_zh { " (登录以获取身份)" } else { " (login for identity)" }, Style::default().fg(Color::DarkGray)),
            ])),
            chunks[0],
        );
    }

    let label = Paragraph::new(Line::from(vec![
        Span::styled(if is_zh { "粘贴共享链接 " } else { "Paste share URL " }, Style::default().fg(Color::Gray)),
        Span::styled(if is_zh { "(例如 https://relay.../share/abc?token=...)" } else { "(e.g. https://relay.../share/abc?token=...)" }, Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(label, chunks[1]);

    let input_text = d.url_input.text();
    let input_style = if d.connecting {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(format!("\u{25b8} {}", input_text))
        .style(input_style);
    f.render_widget(input, chunks[2]);

    // Live validation hint
    if let Some(ref hint) = d.validation_hint {
        let hint_color = if hint.contains("valid") && hint.contains("Enter") {
            Color::Green
        } else if hint.contains("error") || hint.contains("Missing") {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        f.render_widget(
            Paragraph::new(hint.as_str()).style(Style::default().fg(hint_color)),
            chunks[3],
        );
    }

    if let Some(ref status) = d.status {
        let (color, prefix) = if status.contains("failed") || status.contains("Invalid") {
            (Color::Red, "\u{2716} ")
        } else if d.connecting {
            (Color::Yellow, "\u{25cb} ")
        } else {
            (Color::Green, "\u{2714} ")
        };
        let status_line = Paragraph::new(format!("{}{}", prefix, status))
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: false });
        f.render_widget(status_line, chunks[4]);
    }

    let hint_text = if is_zh {
        if d.connecting { "连接中... (Esc 取消)" } else { "回车: 连接  |  Esc: 取消" }
    } else {
        if d.connecting { "Connecting... (Esc to cancel)" } else { "Enter: connect  |  Esc: cancel" }
    };
    f.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(Color::DarkGray)),
        chunks[6],
    );
}

#[cfg(feature = "pro")]
fn render_disconnect_viewer_dialog(f: &mut Frame, area: Rect, d: &crate::ui::DisconnectViewerDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw(if is_zh { "房间: " } else { "Room: " }),
            Span::styled(&d.room_id, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw(if is_zh { "中继: " } else { "Relay: " }),
            Span::styled(&d.relay_url, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(if is_zh { "您想执行什么操作？" } else { "What would you like to do?" }),
        Line::from(""),
        Line::from(vec![
            if d.selected_option == 0 {
                Span::styled(if is_zh { "> 断开连接 (保留会话)" } else { "> Disconnect (keep session)" }, Style::default().fg(Color::Yellow))
            } else {
                Span::raw(if is_zh { "  断开连接 (保留会话)" } else { "  Disconnect (keep session)" })
            },
        ]),
        Line::from(vec![
            if d.selected_option == 1 {
                Span::styled(if is_zh { "> 断开并删除会话" } else { "> Disconnect and delete session" }, Style::default().fg(Color::Red))
            } else {
                Span::raw(if is_zh { "  断开并删除会话" } else { "  Disconnect and delete session" })
            },
        ]),
        Line::from(vec![
            if d.selected_option == 2 {
                Span::styled(if is_zh { "> 取消" } else { "> Cancel" }, Style::default().fg(Color::Green))
            } else {
                Span::raw(if is_zh { "  取消" } else { "  Cancel" })
            },
        ]),
        Line::from(""),
        Line::from(if is_zh { "使用 ↑/↓ 选择，回车确认" } else { "Use ↑/↓ to select, Enter to confirm" }),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(if is_zh { " 断开观察者会话 " } else { " Disconnect Viewer Session " })
            .border_style(Style::default().fg(Color::Yellow)))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, popup_area);
}

#[cfg(feature = "pro")]
fn render_share_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ShareDialog, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let popup_area = centered_rect(65, 55, area);
    f.render_widget(Clear, popup_area);

    // Get viewers for this session
    let viewers = app.relay_client(&d.session_id)
        .map(|c| c.viewers())
        .unwrap_or_default();

    let title = if is_zh {
        if d.already_sharing && !viewers.is_empty() {
            format!("共享: {} ({} 位观察者)", d.session_title, viewers.len())
        } else {
            format!("共享: {}", d.session_title)
        }
    } else {
        if d.already_sharing && !viewers.is_empty() {
            format!("Share: {} ({} viewer{})", d.session_title, viewers.len(), if viewers.len() == 1 { "" } else { "s" })
        } else {
            format!("Share: {}", d.session_title)
        }
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title);

    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);
    let max_viewer_display = 8; // Cap viewer list to prevent layout overflow
    let viewer_rows = if viewers.is_empty() {
        if d.already_sharing { 1 } else { 0 }
    } else {
        let count = viewers.len().min(max_viewer_display);
        let overflow_row = if viewers.len() > max_viewer_display { 1 } else { 0 };
        (count + 2 + overflow_row) as u16 // +2 for header + spacing
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),          // Permission
            Constraint::Length(2),          // Status
            Constraint::Length(2),          // SSH URL
            Constraint::Length(2),          // Web URL
            Constraint::Length(2),          // Expire
            Constraint::Length(viewer_rows), // Viewers list
            Constraint::Min(1),            // Actions
        ])
        .split(inner_area);

    // Permission line
    let perm_text = if d.already_sharing {
        format!("{}{}", if is_zh { "权限: " } else { "Permission: " }, d.permission)
    } else {
        format!("{}{}{}", if is_zh { "权限: " } else { "Permission: " }, d.permission, if is_zh { " (Tab 切换)" } else { " (Tab to toggle)" })
    };
    f.render_widget(
        Paragraph::new(perm_text).style(Style::default().fg(Color::White)),
        chunks[0],
    );

    // Status — show status_message (loading state) if present
    let status = if let Some(ref msg) = d.status_message {
        // Connection in progress or just completed — show with spinner
        let color = if msg.starts_with('✓') {
            Color::Green
        } else if msg.starts_with('✗') {
            Color::Red
        } else {
            Color::Yellow
        };
        let spinner = if !msg.starts_with('✓') && !msg.starts_with('✗') {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let tick = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() / 120) as usize;
            format!("{} ", frames[tick % frames.len()])
        } else {
            String::new()
        };
        Line::from(Span::styled(
            format!("{}{}", spinner, msg),
            Style::default().fg(color),
        ))
    } else if d.already_sharing {
        Line::from(Span::styled(
            if is_zh { "● 共享中" } else { "● Sharing active" },
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(Span::styled(
            if is_zh { "○ 未共享 — 按回车开始" } else { "○ Not sharing — press Enter to start" },
            Style::default().fg(Color::DarkGray),
        ))
    };
    f.render_widget(Paragraph::new(status), chunks[1]);

    // URL display — prefer relay URL over SSH/web
    // Check inline copy feedback (show for 2 seconds)
    let copy_ok = d.copy_feedback_at
        .map(|t| t.elapsed().as_secs() < 2)
        .unwrap_or(false);

    if let Some(ref relay_url) = d.relay_share_url {
        // Relay mode
        let copy_hint = if copy_ok { " ✓ Copied!" } else { " ('c' to copy)" };
        let mut spans = vec![
            Span::styled("URL: ", Style::default().fg(Color::Cyan)),
            Span::styled(relay_url.as_str(), Style::default().fg(Color::Cyan)),
        ];
        spans.push(Span::styled(copy_hint, Style::default().fg(if copy_ok { Color::Green } else { Color::DarkGray })));
        f.render_widget(
            Paragraph::new(Line::from(spans))
                .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[2],
        );
        f.render_widget(
            Paragraph::new(if is_zh { "模式: WebSocket 中继" } else { "Mode: WebSocket relay" })
                .style(Style::default().fg(Color::DarkGray)),
            chunks[3],
        );
    } else {
        // Tmate mode
        let ssh_line = if let Some(ref url) = d.ssh_url {
            let hint = if copy_ok { " ✓ Copied!" } else { " (press 'c' to copy)" };
            format!("SSH: {}{}", url, hint)
        } else {
            "SSH: -".to_string()
        };
        f.render_widget(
            Paragraph::new(ssh_line)
                .style(Style::default().fg(Color::Cyan))
                .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[2],
        );

        let web_line = if let Some(ref url) = d.web_url {
            format!("Web: {}", url)
        } else {
            "Web: -".to_string()
        };
        f.render_widget(
            Paragraph::new(web_line)
                .style(Style::default().fg(Color::Cyan))
                .wrap(ratatui::widgets::Wrap { trim: true }),
            chunks[3],
        );
    }

    // Expire minutes input
    let mut expire_spans = vec![Span::raw(if is_zh { "过期 (分钟): " } else { "Expire (min): " })];
    expire_spans.extend(render_text_input(&d.expire_minutes, true, Style::default()));
    f.render_widget(Paragraph::new(Line::from(expire_spans)), chunks[4]);

    // Viewers list
    if viewers.is_empty() && d.already_sharing {
        f.render_widget(
            Paragraph::new(if is_zh { "  尚无观察者连接。" } else { "  No viewers connected yet." })
                .style(Style::default().fg(Color::DarkGray)),
            chunks[5],
        );
    } else if !viewers.is_empty() {
        let mut viewer_lines = vec![
            Line::from(Span::styled(
                if is_zh { format!("观察者 ({}):", viewers.len()) } else { format!("Viewers ({}):", viewers.len()) },
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
        ];

        // Get presence data for all viewers
        #[cfg(feature = "pro")]
        let presence_map: std::collections::HashMap<String, crate::pro::collab::protocol::PresenceUpdate> = {
            if let Some(client) = app.relay_client(&d.session_id) {
                client.presence().into_iter().map(|p| (p.viewer_id.clone(), p)).collect()
            } else {
                std::collections::HashMap::new()
            }
        };

        // Sort: RW first, then by join time (earliest first)
        let mut sorted_viewers: Vec<_> = viewers.iter().collect();
        sorted_viewers.sort_by(|a, b| {
            let perm_ord = (a.permission != "rw").cmp(&(b.permission != "rw"));
            perm_ord.then_with(|| a.joined_at.cmp(&b.joined_at))
        });

        for (i, v) in sorted_viewers.iter().enumerate() {
            if i >= max_viewer_display {
                viewer_lines.push(Line::from(Span::styled(
                    if is_zh { format!("  ... 还有 {} 个", viewers.len() - max_viewer_display) } else { format!("  ... and {} more", viewers.len() - max_viewer_display) },
                    Style::default().fg(Color::DarkGray),
                )));
                break;
            }
            let (perm_label, perm_style) = if v.permission == "rw" {
                (" RW ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                (" RO ", Style::default().fg(Color::White).bg(Color::DarkGray))
            };
            let is_selected = d.selected_viewer == Some(i);
            let prefix = if is_selected { "> " } else { "  " };
            let name_style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let mut spans = vec![
                Span::raw(prefix),
                Span::styled(perm_label, perm_style),
                Span::raw(" "),
                Span::styled(&v.display_name, name_style),
            ];

            // Add presence indicator (colored dot + mode + position)
            #[cfg(feature = "pro")]
            if let Some(presence) = presence_map.get(&v.viewer_id) {
                if presence.visible {
                    // Parse color hex to ratatui Color
                    let color = parse_hex_color(&presence.color).unwrap_or(Color::Gray);
                    spans.push(Span::styled(" ●", Style::default().fg(color)));

                    // Show mode (LIVE/SCROLL)
                    let mode_str = match presence.mode.as_str() {
                        "LIVE" => " 🔴",
                        "SCROLL" => " 📜",
                        _ => "",
                    };
                    if !mode_str.is_empty() {
                        spans.push(Span::styled(mode_str, Style::default().fg(Color::DarkGray)));
                    }

                    // Show scroll position if in SCROLL mode
                    if presence.mode == "SCROLL" {
                        if let (Some(top), Some(bottom)) = (presence.top_seq, presence.bottom_seq) {
                            let pos_str = format!(" [{}..{}]", top, bottom);
                            spans.push(Span::styled(pos_str, Style::default().fg(Color::DarkGray)));
                        }
                    }
                } else {
                    spans.push(Span::styled(" 👁️‍🗨️", Style::default().fg(Color::DarkGray)));
                }
            }

            // Show connection duration if available
            if let Some(joined) = v.joined_at {
                let elapsed = joined.elapsed();
                let duration_str = if elapsed.as_secs() < 60 {
                    format!(" ({}s)", elapsed.as_secs())
                } else if elapsed.as_secs() < 3600 {
                    format!(" ({}m)", elapsed.as_secs() / 60)
                } else {
                    format!(" ({}h{}m)", elapsed.as_secs() / 3600, (elapsed.as_secs() % 3600) / 60)
                };
                spans.push(Span::styled(duration_str, Style::default().fg(Color::DarkGray)));
            }

            // Show idle indicator if viewer has been inactive for >5 minutes
            if let Some(last_activity) = v.last_activity {
                let idle_secs = last_activity.elapsed().as_secs();
                if idle_secs > 300 {
                    let idle_str = if idle_secs < 3600 {
                        format!(" [idle {}m]", idle_secs / 60)
                    } else {
                        format!(" [idle {}h]", idle_secs / 3600)
                    };
                    spans.push(Span::styled(idle_str, Style::default().fg(Color::Yellow)));
                }
            }

            if is_selected && v.permission == "rw" {
                spans.push(Span::styled("  [d: revoke]", Style::default().fg(Color::Red)));
            }
            viewer_lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(viewer_lines), chunks[5]);
    }

    // Actions hint — show connecting state or normal actions
    let is_connecting = d.status_message.as_ref().is_some_and(|m| !m.starts_with('✓') && !m.starts_with('✗'));
    let action = if is_connecting {
        if is_zh { "连接中... 请稍候  |  Esc: 取消" } else { "Connecting... please wait  |  Esc: Cancel" }
    } else if is_zh {
        if d.already_sharing {
            "回车: 停止  |  c: 复制链接  |  ↑/↓: 观察者  |  Esc: 关闭"
        } else {
            "回车: 开始共享  |  Esc: 关闭"
        }
    } else {
        if d.already_sharing {
            "Enter: Stop  |  c: Copy URL  |  Up/Down: viewers  |  Esc: Close"
        } else {
            "Enter: Start sharing  |  Esc: Close"
        }
    };
    f.render_widget(
        Paragraph::new(action)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[6],
    );
}

#[cfg(feature = "pro")]
fn render_control_request_dialog(f: &mut Frame, area: Rect, d: &crate::ui::ControlRequestDialog, is_zh: bool) {
    let popup_area = centered_rect(50, 30, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .title(if is_zh { "⚠ 控制请求 ⚠" } else { "⚠ Control Request ⚠" });

    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spacing
            Constraint::Length(2), // Request message
            Constraint::Length(2), // Session info
            Constraint::Length(1), // Spacing
            Constraint::Min(1),   // Actions
        ])
        .split(inner_area);

    let requester = if d.display_name.is_empty() {
        if is_zh { "匿名观察者".to_string() } else { "An anonymous viewer".to_string() }
    } else {
        format!("\"{}\"", d.display_name)
    };
    f.render_widget(
        Paragraph::new(if is_zh {
            format!("{} 请求读写控制", requester)
        } else {
            format!("{} requests read-write control", requester)
        })
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(if is_zh { format!("会话 \"{}\"", d.session_title) } else { format!("of session \"{}\"", d.session_title) })
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[2],
    );

    f.render_widget(
        Paragraph::new(if is_zh { "[Y] 批准    [N] 拒绝" } else { "[Y] Approve    [N] Deny" })
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center),
        chunks[4],
    );
}

#[cfg(feature = "pro")]
fn render_orphaned_rooms_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::dialogs::OrphanedRoomsDialog,
    is_zh: bool,
) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let title = if is_zh {
        "⚠ 发现孤立房间 ⚠"
    } else {
        "⚠ Orphaned Rooms Found ⚠"
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .title(title);

    let inner_area = outer.inner(popup_area);
    f.render_widget(outer, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Description
            Constraint::Length(1), // Spacing
            Constraint::Min(1),   // Room list
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Keybind hints
        ])
        .split(inner_area);

    let desc = if is_zh {
        "以下房间来自上次会话，仍在中继服务器上运行："
    } else {
        "These rooms from a previous session are still alive on the relay server:"
    };
    f.render_widget(
        Paragraph::new(desc)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    // Room list
    let items: Vec<ListItem> = d
        .rooms
        .iter()
        .enumerate()
        .map(|(i, room)| {
            let viewers_label = if is_zh {
                format!("{} 个观察者", room.viewer_count)
            } else {
                format!(
                    "{} viewer{}",
                    room.viewer_count,
                    if room.viewer_count == 1 { "" } else { "s" }
                )
            };
            let age = format_room_age(&room.created_at, is_zh);

            let marker = if i == d.selected_index { "▸ " } else { "  " };
            let style = if i == d.selected_index {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(&room.session_id, style),
                Span::raw("  "),
                Span::styled(viewers_label, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(age, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, chunks[2]);

    let hints = if is_zh {
        "Enter: 关闭  a: 全部关闭  Esc: 忽略"
    } else {
        "Enter: Close  a: Close All  Esc: Dismiss"
    };
    f.render_widget(
        Paragraph::new(hints)
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center),
        chunks[4],
    );
}

/// Format room age as a human-readable string (e.g. "5 min ago").
#[cfg(feature = "pro")]
fn format_room_age(created_at: &str, is_zh: bool) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created_at) else {
        return String::new();
    };
    let elapsed = chrono::Utc::now().signed_duration_since(dt);
    let mins = elapsed.num_minutes();
    if mins < 1 {
        if is_zh { "刚刚".to_string() } else { "just now".to_string() }
    } else if mins < 60 {
        if is_zh {
            format!("{} 分钟前", mins)
        } else {
            format!("{} min ago", mins)
        }
    } else {
        let hours = mins / 60;
        if is_zh {
            format!("{} 小时前", hours)
        } else {
            format!("{} hr ago", hours)
        }
    }
}

#[cfg(feature = "pro")]
fn render_create_relationship_dialog(
    f: &mut Frame,
    area: Rect,
    d: &crate::ui::CreateRelationshipDialog,
    is_zh: bool,
) {
    let popup_area = centered_rect(65, 55, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(if is_zh { "新建关系" } else { "New Relationship" });
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
        Paragraph::new(format!("{}{}", if is_zh { "来自: " } else { "From: " }, d.session_a_title))
            .style(Style::default().fg(Color::Cyan)),
        chunks[0],
    );

    // Relation type
    f.render_widget(
        Paragraph::new(format!("{}{}{}", if is_zh { "类型: " } else { "Type: " }, d.relation_type, if is_zh { " (Tab 切换)" } else { " (Tab to cycle)" }))
            .style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );

    // Search input
    let mut search_spans = vec![Span::raw(if is_zh { "搜索: " } else { "Search: " })];
    search_spans.extend(render_text_input(
        &d.search_input,
        is_search,
        Style::default(),
    ));
    f.render_widget(
        Paragraph::new(Line::from(search_spans))
            .block(Block::default().borders(Borders::ALL).title(if is_zh {
                if is_search { "搜索 (活跃)" } else { "搜索" }
            } else {
                if is_search { "Search (active)" } else { "Search" }
            })),
        chunks[2],
    );

    // Session matches
    let items: Vec<ListItem> = if d.matches.is_empty() {
        vec![ListItem::new(Span::styled(
            if is_zh { "  没有匹配的会话" } else { "  No matching sessions" },
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
            .title(if is_zh { "选择目标会话" } else { "Select Target Session" }),
    );
    f.render_widget(list, chunks[3]);

    // Label input
    let mut label_spans = vec![Span::raw(if is_zh { "标签: " } else { "Label: " })];
    label_spans.extend(render_text_input(
        &d.label,
        !is_search,
        Style::default(),
    ));
    f.render_widget(
        Paragraph::new(Line::from(label_spans)).block(
            Block::default().borders(Borders::ALL).title(if is_zh {
                if !is_search { "标签 (活跃)" } else { "标签 (可选)" }
            } else {
                if !is_search { "Label (active)" } else { "Label (optional)" }
            }),
        ),
        chunks[4],
    );

    // Actions
    f.render_widget(
        Paragraph::new(if is_zh { "回车: 创建  |  Tab: 切换类型  |  Shift+Tab: 切换字段  |  Esc: 取消" } else { "Enter: Create  |  Tab: Cycle type  |  Shift+Tab: Switch field  |  Esc: Cancel" })
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[5],
    );
}

#[cfg(feature = "pro")]
fn render_annotate_dialog(f: &mut Frame, area: Rect, d: &crate::ui::AnnotateDialog, is_zh: bool) {
    let popup_area = centered_rect(60, 30, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(if is_zh { "标注关系" } else { "Annotate Relationship" });
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
        Paragraph::new(if is_zh { "为此关系添加备注：" } else { "Add a note to this relationship:" })
            .style(Style::default().fg(Color::White)),
        chunks[0],
    );

    let note_spans = render_text_input(&d.note, true, Style::default());
    f.render_widget(
        Paragraph::new(Line::from(note_spans))
            .block(Block::default().borders(Borders::ALL).title(if is_zh { "备注" } else { "Note" })),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(if is_zh { "回车: 保存  |  Esc: 取消" } else { "Enter: Save  |  Esc: Cancel" })
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
    is_zh: bool,
) {
    let popup_area = centered_rect(70, 60, area);
    f.render_widget(Clear, popup_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(if is_zh { "从上下文新建会话" } else { "New Session from Context" });
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
            .block(Block::default().borders(Borders::ALL).title(if is_zh { "会话标题" } else { "Session Title" })),
        chunks[0],
    );

    // Injection method
    f.render_widget(
        Paragraph::new(format!("{}{}{}", if is_zh { "注入方式: " } else { "Injection: " }, d.injection_method, if is_zh { " (Tab 切换)" } else { " (Tab to cycle)" }))
            .style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );

    // Context preview
    f.render_widget(
        Paragraph::new(d.context_preview.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(if is_zh { "上下文预览" } else { "Context Preview" }),
            )
            .wrap(Wrap { trim: false }),
        chunks[2],
    );

    // Actions
    f.render_widget(
        Paragraph::new(if is_zh { "回车: 创建  |  Tab: 切换方式  |  Esc: 取消" } else { "Enter: Create  |  Tab: Cycle method  |  Esc: Cancel" })
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        chunks[3],
    );
}

/// Render toast notifications as an overlay in the top-right corner.
#[cfg(feature = "pro")]
fn render_toast_notifications(f: &mut Frame, area: Rect, app: &App) {
    let toasts = app.toast_notifications();
    if toasts.is_empty() {
        return;
    }

    // Show up to 3 most recent notifications
    let visible: Vec<_> = toasts.iter().rev().take(3).collect();
    let toast_width = visible.iter()
        .map(|t| t.message.len() as u16 + 4)
        .max()
        .unwrap_or(20)
        .min(area.width / 2);

    let x = area.right().saturating_sub(toast_width + 1);
    let y = area.y + 4; // Below title bar

    for (i, toast) in visible.iter().enumerate() {
        let toast_area = Rect::new(x, y + i as u16, toast_width, 1);
        if toast_area.y < area.bottom() {
            f.render_widget(Clear, toast_area);
            let para = Paragraph::new(format!(" {} ", toast.message))
                .style(Style::default().fg(Color::White).bg(toast.color));
            f.render_widget(para, toast_area);
        }
    }
}

/// Render the viewer mode — displays a shared terminal session received via relay.
#[cfg(feature = "pro")]
fn render_viewer_mode(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let Some(vs) = app.viewer_state() else {
        let msg = Paragraph::new(if is_zh { "未连接到任何共享会话。" } else { "Not connected to any shared session." })
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    };

    let connected = vs.connected.load(std::sync::atomic::Ordering::Relaxed);
    let reconnecting = vs.reconnecting.load(std::sync::atomic::Ordering::Relaxed);
    let has_rw = vs.has_rw_control.load(std::sync::atomic::Ordering::Acquire);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Terminal content
            Constraint::Length(1), // Scroll indicator (when scrolled back)
            Constraint::Length(1), // Presence bar
        ])
        .split(area);

    // --- Terminal content via vt100 parser ---
    let content = vs.terminal_content.lock().unwrap();
    let (host_cols, host_rows) = *vs.host_terminal_size.lock().unwrap();

    // Use HOST dimensions for the parser so tmux capture-pane line structure is
    // preserved correctly.  capture-pane -p -e outputs pre-formatted text with
    // hard newlines at the host's pane width; replaying through a parser of a
    // different width causes wrong wraps and offset rendering.  The viewer clips
    // the rendered output to its own widget size.
    let viewer_cols = chunks[0].width.saturating_sub(2).max(1) as usize;
    let viewer_rows = chunks[0].height.saturating_sub(2).max(1) as usize;
    let parser_cols = if host_cols > 0 { host_cols } else { viewer_cols as u16 };
    let parser_rows = if host_rows > 0 { host_rows } else { viewer_rows as u16 };
    let mut parser = vt100::Parser::new(parser_rows, parser_cols, 2000);
    parser.process(&content);
    drop(content);

    // Apply scroll offset — this shifts what screen.cell() considers "visible"
    let scroll_offset = vs.scroll_offset;
    parser.set_scrollback(scroll_offset);

    let screen = parser.screen();
    let (screen_rows, screen_cols) = screen.size();
    let inner_height = chunks[0].height.saturating_sub(2) as usize; // subtract borders
    // Clip to the narrower of host columns and viewer widget width
    let render_cols = (screen_cols as usize).min(viewer_cols);
    // When viewer is shorter than host, show the bottom portion (most recent content)
    let row_offset = if (screen_rows as usize) > inner_height {
        (screen_rows as usize) - inner_height
    } else {
        0
    };
    let render_rows = inner_height.min(screen_rows as usize);

    // Cursor position — only show when following (scroll_offset == 0) and connected
    // Adjust for row_offset so cursor highlights correctly in the clipped view
    let cursor_pos = if scroll_offset == 0 && connected {
        let (cr, cc) = screen.cursor_position();
        let cr = cr as usize;
        let cc = cc as usize;
        if cr >= row_offset && cr < row_offset + render_rows {
            Some((cr - row_offset, cc))
        } else {
            None
        }
    } else {
        None
    };

    let mut visible_lines: Vec<Line> = Vec::with_capacity(render_rows);
    for i in 0..render_rows {
        let row_idx = i + row_offset; // Map viewer row to host screen row
        let mut spans: Vec<Span> = Vec::new();
        let mut current_text = String::new();
        let mut current_style = Style::default();

        for col_idx in 0..render_cols {
            let cell = screen.cell(row_idx as u16, col_idx as u16);
            if let Some(cell) = cell {
                if cell.is_wide_continuation() {
                    continue; // Skip continuation cells of wide characters
                }
                let mut style = vt100_cell_to_style(cell);
                let ch = cell.contents();

                // Highlight cursor position — RW gets brighter cursor
                if cursor_pos == Some((i, col_idx)) {
                    if has_rw {
                        style = style.fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
                    } else {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                }

                if style == current_style {
                    if ch.is_empty() {
                        current_text.push(' ');
                    } else {
                        current_text.push_str(&ch);
                    }
                } else {
                    if !current_text.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut current_text), current_style));
                    }
                    current_style = style;
                    if ch.is_empty() {
                        current_text.push(' ');
                    } else {
                        current_text.push_str(&ch);
                    }
                }
            }
        }
        if !current_text.is_empty() {
            spans.push(Span::styled(current_text, current_style));
        }
        visible_lines.push(Line::from(spans));
    }

    let reconnect_num = vs.reconnect_attempt.load(std::sync::atomic::Ordering::Relaxed);
    let title = if reconnecting {
        if reconnect_num > 0 {
            format!(" {} [reconnecting {}/10...] ", vs.session_name, reconnect_num)
        } else {
            format!(" {} [reconnecting...] ", vs.session_name)
        }
    } else if !connected && reconnect_num == 0 {
        // Initial connection attempt (never connected yet)
        format!(" {} [connecting...] ", vs.session_name)
    } else if !connected {
        format!(" {} [disconnected] ", vs.session_name)
    } else if has_rw {
        format!(" {} [RW] ", vs.session_name)
    } else {
        format!(" {} [RO] ", vs.session_name)
    };

    let border_color = if reconnecting {
        Color::Yellow
    } else if !connected {
        Color::Red
    } else if has_rw {
        Color::LightCyan
    } else {
        Color::Cyan
    };

    let terminal_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let terminal_paragraph = Paragraph::new(visible_lines).block(terminal_block);
    f.render_widget(terminal_paragraph, chunks[0]);

    // --- Scroll indicator ---
    if scroll_offset > 0 {
        let scroll_text = format!(
            " Scrolled: +{} lines from bottom | End/G: follow | Up/Down/PgUp/PgDn: scroll ",
            scroll_offset,
        );
        let scroll_bar = Paragraph::new(scroll_text)
            .style(Style::default().fg(Color::Black).bg(Color::DarkGray));
        f.render_widget(scroll_bar, chunks[1]);
    }

    // --- Presence bar ---
    let viewer_count = vs.viewer_count.load(std::sync::atomic::Ordering::Relaxed);
    let control_pending = vs.control_requested.load(std::sync::atomic::Ordering::Relaxed);
    // Auto-clear status messages after 5 seconds
    let status_msg: Option<String> = {
        let mut guard = vs.status_message.lock().unwrap();
        match guard.as_ref() {
            Some((_, ts)) if ts.elapsed() > std::time::Duration::from_secs(5) => {
                *guard = None;
                None
            }
            Some((msg, _)) => Some(msg.clone()),
            None => None,
        }
    };

    let status_text = if reconnecting {
        let spinner = ['|', '/', '-', '\\'];
        let spin_char = spinner[app.tick_count() as usize / 2 % spinner.len()];
        if reconnect_num > 0 {
            if is_zh {
                format!("  {} 重连 {} (尝试 {}/10)  |  Esc: 断开", spin_char, vs.session_name, reconnect_num)
            } else {
                format!("  {} Reconnecting to {} (attempt {}/10)  |  Esc: disconnect", spin_char, vs.session_name, reconnect_num)
            }
        } else {
            if is_zh {
                format!("  {} 重连 {}...  |  Esc: 断开", spin_char, vs.session_name)
            } else {
                format!("  {} Reconnecting to {}...  |  Esc: disconnect", spin_char, vs.session_name)
            }
        }
    } else if connected {
        let identity_part = match &vs.viewer_identity {
            Some(name) => format!(" ({})", name),
            None => if is_zh { " (匿名)".to_string() } else { " (anonymous)".to_string() },
        };
        let peers = vs.peer_viewers.read().unwrap_or_else(|e| e.into_inner());
        let peer_names: String = if peers.is_empty() {
            String::new()
        } else {
            // Sort: RW first, then by join time (earliest first)
            #[cfg(feature = "pro")]
            let names: Vec<&str> = {
                let mut sorted: Vec<_> = peers.iter().collect();
                sorted.sort_by(|a, b| {
                    let perm_ord = (a.permission != "rw").cmp(&(b.permission != "rw"));
                    perm_ord.then_with(|| a.joined_at.cmp(&b.joined_at))
                });
                sorted.iter().take(3).map(|v| v.display_name.as_str()).collect()
            };
            #[cfg(not(feature = "pro"))]
            let names: Vec<&str> = peers.iter().take(3).map(|v| v.as_str()).collect();
            let suffix = if peers.len() > 3 { format!(" +{}", peers.len() - 3) } else { String::new() };
            format!(" ({}{})", names.join(", "), suffix)
        };
        drop(peers);
        let viewers_part = if is_zh {
            format!(
                "  {}  {}{}  |  {} 位观察者{}",
                connection_pulse(app.tick_count()),
                vs.session_name,
                identity_part,
                viewer_count,
                peer_names,
            )
        } else {
            format!(
                "  {}  {}{}  |  {} viewer{}{}",
                connection_pulse(app.tick_count()),
                vs.session_name,
                identity_part,
                viewer_count,
                if viewer_count == 1 { "" } else { "s" },
                peer_names,
            )
        };

        let control_part = if has_rw {
            if is_zh {
                "  |  读写中  |  Esc: 释放  |  Shift+方向键/PgUp/PgDn: 滚动".to_string()
            } else {
                "  |  RW active  |  Esc: relinquish  |  Shift+Arrows/PgUp/PgDn: scroll".to_string()
            }
        } else if let Some(ref msg) = status_msg {
            format!("  |  {}", msg)
        } else if control_pending {
            let dots = ".".repeat((app.tick_count() as usize / 2 % 3) + 1);
            if is_zh {
                format!("  |  已请求控制{}", dots)
            } else {
                format!("  |  control requested{}", dots)
            }
        } else {
            if is_zh { "  |  r: 请求控制".to_string() } else { "  |  r: request control".to_string() }
        };

        let latency = vs.latency_ms.load(std::sync::atomic::Ordering::Relaxed);
        let latency_part = if latency > 0 {
            format!("  |  {} {}ms", connection_quality_icon(latency), latency)
        } else {
            String::new()
        };

        // Bandwidth display
        let bytes_rx = vs.bytes_received_per_sec.load(std::sync::atomic::Ordering::Relaxed);
        let bytes_tx = vs.bytes_sent_per_sec.load(std::sync::atomic::Ordering::Relaxed);
        let bandwidth_part = if bytes_rx > 0 || bytes_tx > 0 {
            format!("  |  ↓{} ↑{}", format_bandwidth(bytes_rx), format_bandwidth(bytes_tx))
        } else {
            String::new()
        };

        let elapsed = vs.connected_at.elapsed().as_secs();
        let duration_part = if elapsed >= 3600 {
            format!("  |  {}h{}m", elapsed / 3600, (elapsed % 3600) / 60)
        } else if elapsed >= 60 {
            format!("  |  {}m", elapsed / 60)
        } else {
            format!("  |  {}s", elapsed)
        };
        // Build status bar with width-aware truncation
        // Use char count (not byte length) for better Unicode width estimation
        let esc_part = if is_zh { "  |  Esc: 断开" } else { "  |  Esc: disconnect" };
        let available = area.width as usize;

        // Priority order: viewers + control (essential), latency (important), bandwidth (nice-to-have), duration (nice-to-have)
        let essential = format!("{}{}{}", viewers_part, control_part, esc_part);
        if essential.chars().count() >= available {
            // Ultra-narrow: just show session name + control
            format!("  {}{}  |  Esc", vs.session_name, control_part)
        } else {
            let with_latency = format!("{}{}{}{}", viewers_part, control_part, latency_part, esc_part);
            if with_latency.chars().count() >= available {
                essential
            } else {
                let with_bw = format!("{}{}{}{}{}", viewers_part, control_part, latency_part, bandwidth_part, esc_part);
                if with_bw.chars().count() >= available {
                    with_latency
                } else {
                    let full = format!("{}{}{}{}{}{}", viewers_part, control_part, latency_part, bandwidth_part, duration_part, esc_part);
                    if full.chars().count() >= available {
                        with_bw
                    } else {
                        full
                    }
                }
            }
        }
    } else {
        if is_zh {
            format!("  已断开 {}  |  按 Esc 返回", vs.session_name)
        } else {
            format!("  Disconnected from {}  |  Press Esc to return", vs.session_name)
        }
    };

    let status_color = if reconnecting {
        Color::Yellow
    } else if connected {
        if has_rw {
            Color::Cyan
        } else if status_msg.as_ref().is_some_and(|m| m.contains("denied")) {
            Color::Yellow
        } else {
            Color::Green
        }
    } else {
        Color::Red
    };
    let presence_bar = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White).bg(status_color));

    f.render_widget(presence_bar, chunks[2]);

    // Help overlay (toggled with '?')
    if vs.show_help {
        let help_lines = vec![
            Line::from(Span::styled(if is_zh { "键盘快捷键" } else { "Keyboard Shortcuts" }, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(vec![
                Span::styled("  r       ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "请求读写控制" } else { "Request read-write control" }),
            ]),
            Line::from(vec![
                Span::styled("  Esc     ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { if has_rw { "放弃读写控制" } else { "断开连接" } } else { if has_rw { "Relinquish RW control" } else { "Disconnect" } }),
            ]),
            Line::from(vec![
                Span::styled("  q       ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "断开连接 (只读模式)" } else { "Disconnect (RO mode)" }),
            ]),
            Line::from(""),
            Line::from(Span::styled(if is_zh { "滚动" } else { "Scroll" }, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(vec![
                Span::styled("  Up/k    ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "向上滚动" } else { "Scroll up" }),
            ]),
            Line::from(vec![
                Span::styled("  Down/j  ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "向下滚动" } else { "Scroll down" }),
            ]),
            Line::from(vec![
                Span::styled("  PgUp    ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "向上翻页" } else { "Page up" }),
            ]),
            Line::from(vec![
                Span::styled("  PgDn    ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "向下翻页" } else { "Page down" }),
            ]),
            Line::from(vec![
                Span::styled("  Home    ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "滚动到顶部" } else { "Scroll to top" }),
            ]),
            Line::from(vec![
                Span::styled("  End/G   ", Style::default().fg(Color::Cyan)),
                Span::raw(if is_zh { "滚动到底部" } else { "Scroll to bottom" }),
            ]),
            Line::from(""),
            Line::from(Span::styled(if is_zh { "连接状态" } else { "Connection Stats" }, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        ];
        // Add connection stats
        let mut help_lines = help_lines;
        if let Ok(stats) = vs.connection_stats.lock() {
            let (min, max, avg, samples) = *stats;
            if samples > 0 {
                help_lines.push(Line::from(vec![
                    Span::styled("  Latency ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("avg {}ms  min {}ms  max {}ms", avg, min, max)),
                ]));
                help_lines.push(Line::from(vec![
                    Span::styled("  Samples ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{}", samples)),
                ]));
            } else {
                help_lines.push(Line::from(Span::styled(if is_zh { "  暂无数据" } else { "  No data yet" }, Style::default().fg(Color::DarkGray))));
            }
        }
        let elapsed = vs.connected_at.elapsed().as_secs();
        let uptime_str = if elapsed >= 3600 {
            format!("{}h {}m", elapsed / 3600, (elapsed % 3600) / 60)
        } else if elapsed >= 60 {
            format!("{}m {}s", elapsed / 60, elapsed % 60)
        } else {
            format!("{}s", elapsed)
        };
        help_lines.push(Line::from(vec![
            Span::styled("  Uptime  ", Style::default().fg(Color::Cyan)),
            Span::raw(uptime_str),
        ]));
        help_lines.push(Line::from(vec![
            Span::styled(if is_zh { "  主机终端" } else { "  Host    " }, Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}x{}", host_cols, host_rows)),
        ]));

        // Peer viewers section
        let peers = vs.peer_viewers.read().unwrap_or_else(|e| e.into_inner());
        if !peers.is_empty() {
            help_lines.push(Line::from(""));
            help_lines.push(Line::from(Span::styled(
                if is_zh { format!("对等方 ({})", peers.len()) } else { format!("Peers ({})", peers.len()) },
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            // Sort: RW first, then by join time (earliest first)
            #[cfg(feature = "pro")]
            let sorted_peers: Vec<_> = {
                let mut s: Vec<_> = peers.iter().collect();
                s.sort_by(|a, b| {
                    let perm_ord = (a.permission != "rw").cmp(&(b.permission != "rw"));
                    perm_ord.then_with(|| a.joined_at.cmp(&b.joined_at))
                });
                s
            };
            #[cfg(feature = "pro")]
            for v in sorted_peers.iter().take(5) {
                let (perm_label, perm_style) = if v.permission == "rw" {
                    ("RW", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    ("RO", Style::default().fg(Color::DarkGray))
                };
                let mut spans = vec![
                    Span::raw("  "),
                    Span::styled(perm_label, perm_style),
                    Span::raw(" "),
                    Span::styled(v.display_name.as_str(), Style::default().fg(Color::White)),
                ];
                // Show how long this peer has been connected
                if let Some(joined) = v.joined_at {
                    let secs = joined.elapsed().as_secs();
                    let dur = if secs >= 3600 {
                        format!(" {}h", secs / 3600)
                    } else if secs >= 60 {
                        format!(" {}m", secs / 60)
                    } else {
                        format!(" {}s", secs)
                    };
                    spans.push(Span::styled(dur, Style::default().fg(Color::DarkGray)));
                }
                help_lines.push(Line::from(spans));
            }
            #[cfg(not(feature = "pro"))]
            for v in peers.iter().take(5) {
                help_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(v.as_str(), Style::default().fg(Color::White)),
                ]));
            }
            if peers.len() > 5 {
                help_lines.push(Line::from(Span::styled(
                    if is_zh { format!("  ... 还有 {} 个", peers.len() - 5) } else { format!("  ... and {} more", peers.len() - 5) },
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
        // peers guard is automatically dropped here

        help_lines.push(Line::from(""));
        help_lines.push(Line::from(Span::styled("  ?       ", Style::default().fg(Color::DarkGray))));
        // Last line: close hint
        if let Some(last) = help_lines.last_mut() {
            *last = Line::from(vec![
                Span::styled("  ?       ", Style::default().fg(Color::DarkGray)),
                Span::raw(if is_zh { "关闭帮助" } else { "Close this help" }),
            ]);
        }

        let help_height = help_lines.len() as u16 + 2; // +2 for borders
        let help_width = 48;
        let popup = centered_rect_fixed(help_width, help_height, area);
        f.render_widget(Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(if is_zh { " 帮助 " } else { " Help " });
        let inner = block.inner(popup);
        f.render_widget(block, popup);
        f.render_widget(Paragraph::new(help_lines), inner);
    }
}

/// Create a centered rectangle with fixed width and height.
#[cfg(feature = "pro")]
fn centered_rect_fixed(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}

/// Convert a vt100 cell's attributes to a ratatui Style.
#[cfg(feature = "pro")]
fn vt100_cell_to_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default();

    // Foreground color
    style = style.fg(vt100_color_to_ratatui(cell.fgcolor()));

    // Background color
    let bg = cell.bgcolor();
    if !matches!(bg, vt100::Color::Default) {
        style = style.bg(vt100_color_to_ratatui(bg));
    }

    // Text attributes
    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    style
}

/// Map vt100 colors to ratatui colors.
#[cfg(feature = "pro")]
fn vt100_color_to_ratatui(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(0) => Color::Black,
        vt100::Color::Idx(1) => Color::Red,
        vt100::Color::Idx(2) => Color::Green,
        vt100::Color::Idx(3) => Color::Yellow,
        vt100::Color::Idx(4) => Color::Blue,
        vt100::Color::Idx(5) => Color::Magenta,
        vt100::Color::Idx(6) => Color::Cyan,
        vt100::Color::Idx(7) => Color::White,
        vt100::Color::Idx(8) => Color::DarkGray,
        vt100::Color::Idx(9) => Color::LightRed,
        vt100::Color::Idx(10) => Color::LightGreen,
        vt100::Color::Idx(11) => Color::LightYellow,
        vt100::Color::Idx(12) => Color::LightBlue,
        vt100::Color::Idx(13) => Color::LightMagenta,
        vt100::Color::Idx(14) => Color::LightCyan,
        vt100::Color::Idx(15) => Color::White,
        vt100::Color::Idx(idx) => Color::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
