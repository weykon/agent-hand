use super::*;
use crate::agent::guard::{EvidenceRecord, FeedbackPacket, GuardedCommit};
use crate::agent::projections::{
    build_evidence_view_model, build_relationship_view_model, build_scheduler_view_model,
    build_workflow_view_model,
};
use crate::agent::scheduler::SchedulerState;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
struct DerivedHookRecord {
    #[allow(dead_code)]
    r#type: String,
    #[serde(default)]
    tmux_session: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    ts: f64,
}

const TOKEN_BURST_FRESH_SECS: f64 = 8.0;

fn token_burst_anim(tick: u64) -> &'static str {
    if tick % 2 == 0 { "✦" } else { "✧" }
}

fn load_recent_token_bursts(runtime_dir: &Path) -> Vec<DerivedHookRecord> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    load_jsonl::<DerivedHookRecord>(&runtime_dir.join("derived_hooks.jsonl"))
        .into_iter()
        .filter(|r| now - r.ts <= TOKEN_BURST_FRESH_SECS)
        .collect()
}

fn session_has_recent_token_burst(
    session: &crate::session::Instance,
    bursts: &[DerivedHookRecord],
) -> bool {
    let cli_sid = session.cli_session_id();
    let tmux_name = session.tmux_name();
    bursts.iter().any(|r| {
        cli_sid.is_some_and(|sid| !r.session_id.is_empty() && r.session_id == sid)
            || (!r.tmux_session.is_empty() && r.tmux_session == tmux_name)
    })
}

/// Render session list (splits off active panel at top when premium + active sessions exist)
pub(super) fn render_session_list(f: &mut Frame, area: Rect, app: &App) {
    #[cfg(feature = "pro")]
    {
        let is_pro = app.auth_token().map_or(false, |t| t.is_pro());
        let active = app.active_sessions();
        let has_viewer_sessions = !&app.pro.viewer_sessions.is_empty();

        if is_pro && (!active.is_empty() || has_viewer_sessions) {
            // Calculate heights for active panel and viewer sessions panel
            let max_h = (area.height * 2 / 5).max(8);
            let active_panel_h = if !active.is_empty() {
                (active.len() as u16 + 2).min(max_h)
            } else {
                0
            };
            let viewer_panel_h = if has_viewer_sessions {
                (app.pro.viewer_sessions.len() as u16 + 2).min(max_h)
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
pub(super) fn render_active_panel(f: &mut Frame, area: Rect, app: &App, active: &[&crate::session::Instance]) {
    let focused = app.active_panel_focused();
    let selected = app.active_panel_selected();
    let token_bursts = load_recent_token_bursts(app.runtime_dir());

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

            if session_has_recent_token_burst(s, &token_bursts) {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    token_burst_anim(app.tick_count()),
                    if is_selected {
                        base.add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::LightRed)
                            .add_modifier(Modifier::BOLD)
                    },
                ));
                spans.push(Span::styled(
                    " tok",
                    if is_selected {
                        base
                    } else {
                        Style::default().fg(Color::LightRed)
                    },
                ));
            }

            // Show sharing indicator with viewer count
            #[cfg(feature = "pro")]
            if let Some(ref sharing) = s.sharing {
                if sharing.active {
                    if let Some(relay) = app.relay_client(&s.id) {
                        if !relay.is_connected() {
                            // Host WS disconnected — show lost state
                            spans.push(Span::styled(
                                " ✕ disconnected",
                                if is_selected { base } else { Style::default().fg(Color::Red) },
                            ));
                        } else {
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
pub(super) fn render_viewer_sessions_panel(f: &mut Frame, area: Rect, app: &App) {
    let sessions = &app.pro.viewer_sessions;
    let focused = app.pro.viewer_panel_focused;
    let selected = app.pro.viewer_panel_selected;

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

            // Show session name if available, otherwise truncated room_id
            let display_name = info.session_name.as_deref().unwrap_or(room_id);
            let display_name = if display_name.len() > 20 {
                format!("{}...", &display_name[..20])
            } else {
                display_name.to_string()
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
                Span::styled(display_name, if is_selected { base } else { Style::default().fg(Color::Cyan) }),
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
pub(super) fn render_session_tree(f: &mut Frame, area: Rect, app: &App) {
    let tree = app.tree();
    let token_bursts = load_recent_token_bursts(app.runtime_dir());

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
        { !app.active_panel_focused() && !app.pro.viewer_panel_focused }
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

                    if let Some(session) = s {
                        if session_has_recent_token_burst(session, &token_bursts) {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(
                                format!("{} tok", token_burst_anim(app.tick_count())),
                                Style::default()
                                    .fg(Color::LightRed)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
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
                                        if !relay.is_connected() {
                                            spans.push(Span::styled(
                                                "[share: disconnected]",
                                                Style::default().fg(Color::Red),
                                            ));
                                        } else {
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
                                    } // end else (is_connected)
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

                        // AI analysis badges (Max tier)
                        #[cfg(feature = "max")]
                        {
                            if app.is_summarizing(&session.id) || app.is_diagramming(&session.id) {
                                spans.push(Span::raw("  "));
                                spans.push(Span::styled(
                                    "⏳",
                                    Style::default().fg(Color::Yellow),
                                ));
                            } else {
                                if app.has_ai_summary(&session.id) {
                                    spans.push(Span::raw(" "));
                                    spans.push(Span::styled(
                                        "🤖",
                                        Style::default().fg(Color::Cyan),
                                    ));
                                }
                                if app.has_ai_diagram(&session.id) {
                                    spans.push(Span::raw(" "));
                                    spans.push(Span::styled(
                                        "📊",
                                        Style::default().fg(Color::Magenta),
                                    ));
                                }
                            }
                        }
                    }

                    let line = Line::from(spans);
                    ListItem::new(line)
                }
                TreeItem::Relationship { id, rel_id, depth } => {
                    let indent = "  ".repeat(*depth);
                    let s = app.session_by_id(id);

                    let (status_icon, status_color, title) = if let Some(session) = s {
                        let icon = match session.status {
                            Status::Waiting => waiting_anim(app.tick_count()),
                            Status::Running => running_anim(app.tick_count()),
                            Status::Idle => "○",
                            Status::Error => "✕",
                            Status::Starting => "⋯",
                        };
                        let color = match session.status {
                            Status::Waiting => Color::Blue,
                            Status::Running => Color::Yellow,
                            Status::Idle => Color::DarkGray,
                            Status::Error => Color::Red,
                            Status::Starting => Color::Cyan,
                        };
                        (icon, color, session.title.as_str())
                    } else {
                        ("?", Color::Red, "<missing>")
                    };

                    // Determine badge color from relationship type
                    let rel_color = app.relationships().iter()
                        .find(|r| r.id == *rel_id)
                        .map(|r| match r.relation_type {
                            crate::session::RelationType::Peer => Color::Cyan,
                            crate::session::RelationType::Dependency => Color::Yellow,
                            crate::session::RelationType::Collaboration => Color::Green,
                            crate::session::RelationType::ParentChild => Color::Magenta,
                            crate::session::RelationType::Custom => Color::Blue,
                        })
                        .unwrap_or(Color::Blue);

                    let mut spans = vec![
                        Span::styled(indent, Style::default()),
                        Span::styled(status_icon, Style::default().fg(status_color)),
                        Span::raw(" "),
                        Span::styled("⇄ ", Style::default().fg(rel_color)),
                        Span::styled(title, base.fg(rel_color).add_modifier(Modifier::BOLD)),
                    ];

                    // Token burst for relationship sessions too
                    if let Some(session) = s {
                        if session_has_recent_token_burst(session, &token_bursts) {
                            spans.push(Span::raw("  "));
                            spans.push(Span::styled(
                                format!("{} tok", token_burst_anim(app.tick_count())),
                                Style::default()
                                    .fg(Color::LightRed)
                                    .add_modifier(Modifier::BOLD),
                            ));
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

    let mut state = if tree_focused {
        app.list_state().clone()
    } else {
        ListState::default()
    };
    f.render_stateful_widget(list, area, &mut state);
}

pub(super) fn render_preview(f: &mut Frame, area: Rect, app: &App) {
    let preview_label = match app.language() {
        crate::i18n::Language::Chinese => "预览",
        crate::i18n::Language::English => "Preview",
    };
    let title = match app.selected_item() {
        Some(TreeItem::Session { id, .. } | TreeItem::Relationship { id, .. }) => app
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

/// Render the AI summary overlay popup (Max tier).
#[cfg(feature = "max")]
pub(super) fn render_ai_summary_overlay(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::Clear;

    let Some((session_title, summary)) = app.ai_summary_overlay_text() else {
        return;
    };

    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let modal_area = centered_rect(75, 70, area);
    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " 🤖 AI ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    // Split: header (3 lines) | scrollable summary content | footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),   // summary body (scrollable)
            Constraint::Length(2), // footer
        ])
        .split(inner);

    // Header
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                "🤖 AI Summary",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                session_title.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(header_lines), chunks[0]);

    // Summary body with scroll
    let summary_lines: Vec<Line> = summary.lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    let total_lines = summary_lines.len() as u16;
    let visible_height = chunks[1].height;
    let scroll = app.max.summary_overlay_scroll.min(total_lines.saturating_sub(visible_height));

    let summary_para = Paragraph::new(summary_lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(summary_para, chunks[1]);

    // Footer
    let scroll_hint = if total_lines > visible_height {
        format!(" [{}/{}]", scroll + 1, total_lines.saturating_sub(visible_height) + 1)
    } else {
        String::new()
    };
    let footer_lines = vec![
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "[Esc] 关闭  [C] 添加到画布  [j/k] 滚动  [A] 重新分析" }
                else { "[Esc] Close  [C] Add to Canvas  [j/k] Scroll  [A] Re-analyze" },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(scroll_hint, Style::default().fg(Color::Yellow)),
        ]),
    ];
    f.render_widget(Paragraph::new(footer_lines), chunks[2]);
}

/// Render the AI diagram overlay popup (Max tier) with scrollable content.
#[cfg(feature = "max")]
pub(super) fn render_ai_diagram_overlay(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::Clear;

    let Some((session_title, diagram)) = app.ai_diagram_overlay_text() else {
        return;
    };

    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let modal_area = centered_rect(85, 85, area);
    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(Span::styled(
            " 📊 Diagram ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    // Split: header (3 lines) | scrollable diagram content | footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),   // diagram body (scrollable)
            Constraint::Length(2), // footer
        ])
        .split(inner);

    // Header
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                "📊 AI Diagram",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                session_title.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(header_lines), chunks[0]);

    // Diagram body with scroll
    let diagram_lines: Vec<Line> = diagram.lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    let total_lines = diagram_lines.len() as u16;
    let visible_height = chunks[1].height;
    let scroll = app.max.diagram_overlay_scroll.min(total_lines.saturating_sub(visible_height));

    let diagram_para = Paragraph::new(diagram_lines)
        .scroll((scroll, 0));
    f.render_widget(diagram_para, chunks[1]);

    // Footer
    let scroll_hint = if total_lines > visible_height {
        format!(" [{}/{}]", scroll + 1, total_lines.saturating_sub(visible_height) + 1)
    } else {
        String::new()
    };
    let footer_lines = vec![
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "[Esc] 关闭  [C] 添加到画布  [j/k] 滚动  [A] 重新生成" }
                else { "[Esc] Close  [C] Add to Canvas  [j/k] Scroll  [A] Regenerate" },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(scroll_hint, Style::default().fg(Color::Yellow)),
        ]),
    ];
    f.render_widget(Paragraph::new(footer_lines), chunks[2]);
}

/// Render the behavior analysis overlay popup (Max tier) with scrollable content.
#[cfg(feature = "max")]
pub(super) fn render_behavior_overlay(f: &mut Frame, area: Rect, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::Clear;

    let Some((session_title, analysis)) = app.behavior_overlay_text() else {
        return;
    };

    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let modal_area = centered_rect(75, 70, area);
    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(Span::styled(
            if is_zh { " 🧠 行为分析 " } else { " 🧠 Behavior Analysis " },
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    // Split: header (3 lines) | scrollable body | footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),   // analysis body (scrollable)
            Constraint::Length(2), // footer
        ])
        .split(inner);

    // Header
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                if is_zh { "🧠 用户行为分析" } else { "🧠 Behavior Analysis" },
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                if is_zh { "会话: " } else { "Session: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                session_title.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(header_lines), chunks[0]);

    // Analysis body with scroll
    let body_lines: Vec<Line> = analysis.lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    let total_lines = body_lines.len() as u16;
    let visible_height = chunks[1].height;
    let scroll = app.max.behavior_overlay_scroll.min(total_lines.saturating_sub(visible_height));

    let body_para = Paragraph::new(body_lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(body_para, chunks[1]);

    // Footer
    let scroll_hint = if total_lines > visible_height {
        format!(" [{}/{}]", scroll + 1, total_lines.saturating_sub(visible_height) + 1)
    } else {
        String::new()
    };
    let footer_lines = vec![
        Line::from(vec![
            Span::styled(
                "─".repeat(inner.width.saturating_sub(1) as usize),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "[Esc] 关闭  [j/k] 滚动  [B] 重新分析" }
                else { "[Esc] Close  [j/k] Scroll  [B] Re-analyze" },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(scroll_hint, Style::default().fg(Color::Yellow)),
        ]),
    ];
    f.render_widget(Paragraph::new(footer_lines), chunks[2]);
}

/// Render help as a centered modal overlay
pub(super) fn render_onboarding_welcome(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
    use crate::i18n::Language;

    let modal_area = centered_rect(70, 60, area);
    f.render_widget(Clear, modal_area);

    let (title_str, welcome_title, desc, features_title, f1, f2, f3, f4,
         start_title, s1, s2, s3, s4, continue_str) = match lang {
        Language::Chinese => (
            " 欢迎 ",
            "欢迎使用 Agent Hand！",
            "Agent Hand 帮助您高效管理多个 AI 智能体会话。",
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
            "Welcome to Agent Hand!",
            "Agent Hand helps you manage multiple AI agent sessions efficiently.",
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

pub(super) fn render_help_modal(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
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
        key("u", if is_zh { "恢复：继续 AI CLI 对话" } else { "Resume: continue AI CLI conversation" }),
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
        key("K", if is_zh { "打开 Skills 浏览器 (Pro)" } else { "Open skills browser (Pro)" }),
        key("Shift+S", if is_zh { "通过中继分享会话 (Pro)" } else { "Share session via relay (Pro)" }),
        key("Shift+J", if is_zh { "通过 URL 加入共享会话 (Pro)" } else { "Join a shared session by URL (Pro)" }),
        key(",", if is_zh { "打开设置" } else { "Open settings" }),
        key("?", if is_zh { "切换帮助界面" } else { "Toggle this help screen" }),
        key("q", if is_zh { "退出 Agent Hand" } else { "Quit Agent Hand" }),
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

/// Render keyboard hint spans for normal item selection (legacy inline version).
/// Kept for potential viewer-mode reuse; overlay now uses collect_item_selection_hints.
#[allow(dead_code)]
pub(super) fn render_item_hints(spans: &mut Vec<Span<'static>>, app: &App) {
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
        Some(TreeItem::Session { .. } | TreeItem::Relationship { .. }) => {
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
            #[cfg(feature = "pro")]
            {
                spans.push(Span::styled("a", Style::default().fg(Color::Green)));
                spans.push(Span::raw(if is_zh { ":+画布  " } else { ":+canvas  " }));
            }
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

/// Render the Relationships view (Premium)
#[cfg(feature = "pro")]
pub(super) fn render_relationships(f: &mut Frame, area: Rect, app: &App) {
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

/// Render relationship detail panel below canvas when an edge with relationship_id is selected.
#[cfg(feature = "pro")]
pub(super) fn render_relationship_detail(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let canvas = app.canvas_state();

    let rel_id = match canvas.selected_edge_relationship_id() {
        Some(id) => id,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(if is_zh { " 关系详情 " } else { " Edge Detail " });
            f.render_widget(block, area);
            return;
        }
    };

    // Find the relationship data
    let rel = app.relationships().iter().find(|r| r.id == rel_id);

    let text = if let Some(rel) = rel {
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
            .map(|s| format!("{:?}", s.status).to_lowercase())
            .unwrap_or_else(|| "?".into());
        let b_status = app
            .session_by_id(&rel.session_b_id)
            .map(|s| format!("{:?}", s.status).to_lowercase())
            .unwrap_or_else(|| "?".into());

        let indicator = rel.direction_indicator();
        let label_str = rel.label.as_deref().unwrap_or("");
        let dep_info = if rel.relation_type == crate::session::RelationType::Dependency {
            let source_idle = app
                .session_by_id(&rel.session_a_id)
                .is_some_and(|s| matches!(s.status, crate::session::Status::Idle));
            if source_idle {
                if is_zh { " ✓ 依赖就绪" } else { " ✓ dep ready" }
            } else {
                if is_zh { " ⏳ 等待依赖" } else { " ⏳ waiting" }
            }
        } else {
            ""
        };

        let snapshot_count = app.snapshot_count(&rel.id);

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    if is_zh { "类型: " } else { "Type: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}", rel.relation_type),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(dep_info),
            ]),
            Line::from(vec![
                Span::styled(a_title, Style::default().fg(Color::Green)),
                Span::styled(format!(" ({}) ", a_status), Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} ", indicator)),
                Span::styled(b_title, Style::default().fg(Color::Green)),
                Span::styled(format!(" ({})", b_status), Style::default().fg(Color::DarkGray)),
            ]),
        ];
        if !label_str.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    if is_zh { "标签: " } else { "Label: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(label_str, Style::default().fg(Color::Yellow)),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled(
                if is_zh { "快照: " } else { "Snapshots: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{}", snapshot_count)),
        ]));
        // Action hints
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                if is_zh { "c:捕获 a:标注 d:删除 Ctrl+N:新建会话" }
                else { "c:capture a:annotate d:delete Ctrl+N:new session" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines
    } else {
        vec![Line::from(Span::styled(
            if is_zh { "关系未找到" } else { "Relationship not found" },
            Style::default().fg(Color::Red),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(if is_zh { " 关系详情 " } else { " Edge Detail " });
    let para = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

use crate::agent::io::load_jsonl;

fn selected_runtime_session_id(app: &App) -> Option<String> {
    if app.canvas_focused() {
        if let Some(sid) = app.canvas_state().session_id_at_cursor() {
            return Some(sid);
        }
    }
    match app.selected_item() {
        Some(crate::ui::TreeItem::Session { id, .. } | crate::ui::TreeItem::Relationship { id, .. }) => Some(id.clone()),
        _ => None,
    }
}

pub(super) fn render_canvas_projection_detail(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(if is_zh {
            " 工作流详情 "
        } else {
            " Workflow Detail "
        });

    let Some(runtime_session_id) = selected_runtime_session_id(app) else {
        f.render_widget(block, area);
        return;
    };

    let Some(session) = app.session_by_id(&runtime_session_id) else {
        f.render_widget(block, area);
        return;
    };

    let source_session_id = session
        .cli_session_id()
        .map(|s| s.to_string())
        .unwrap_or_else(|| runtime_session_id.clone());

    let relationship_view = build_relationship_view_model(app.sessions(), app.relationships());

    let packets: Vec<FeedbackPacket> = load_jsonl::<FeedbackPacket>(&app.runtime_dir().join("feedback_packets.jsonl"))
        .into_iter()
        .filter(|p| p.source_session_id == source_session_id)
        .collect();
    let commits: Vec<GuardedCommit> = load_jsonl::<GuardedCommit>(&app.runtime_dir().join("commits.jsonl"));
    let evidence: Vec<EvidenceRecord> = load_jsonl::<EvidenceRecord>(&app.runtime_dir().join("evidence.jsonl"));
    let scheduler_state: SchedulerState = fs::read_to_string(app.runtime_dir().join("scheduler_state.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let scheduler_view = build_scheduler_view_model(&scheduler_state);
    let workflow_view = build_workflow_view_model(&packets, &scheduler_state);

    let packet_trace_ids: std::collections::HashSet<String> =
        packets.iter().map(|p| p.trace_id.clone()).collect();
    let session_commits: Vec<GuardedCommit> = commits
        .into_iter()
        .filter(|c| packet_trace_ids.contains(&c.trace_id))
        .collect();
    let session_evidence: Vec<EvidenceRecord> = evidence
        .into_iter()
        .filter(|e| packet_trace_ids.contains(&e.trace_id))
        .collect();
    let evidence_view = build_evidence_view_model(&session_commits, &session_evidence);

    let relationship_count = relationship_view
        .edges
        .iter()
        .filter(|e| e.source_session_id == runtime_session_id || e.target_session_id == runtime_session_id)
        .count();
    let pending_count = scheduler_view
        .pending_coordination
        .iter()
        .filter(|r| r.source_session_id == source_session_id)
        .count();
    let review_count = scheduler_view
        .review_queue
        .iter()
        .filter(|r| r.source_session_id == source_session_id)
        .count();
    let followup_count = scheduler_view
        .proposed_followups
        .iter()
        .filter(|r| r.source_session_id == source_session_id)
        .count();

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                if is_zh { "会话: " } else { "Session: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(session.title.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(" ({})", format!("{:?}", session.status).to_lowercase()),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "关系数: " } else { "Relations: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{}", relationship_count)),
            Span::raw("  "),
            Span::styled(
                if is_zh { "包: " } else { "Packets: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{}", packets.len())),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "调度: " } else { "Scheduler: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!(
                "pending={} review={} followup={}",
                pending_count, review_count, followup_count
            )),
        ]),
        Line::from(vec![
            Span::styled(
                if is_zh { "证据: " } else { "Evidence: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!(
                "commits={} evidence={}",
                evidence_view.decisions.len(),
                session_evidence.len()
            )),
        ]),
    ];

    if let Some(step) = workflow_view.steps.last() {
        if !step.blockers.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    if is_zh { "阻塞: " } else { "Blocker: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(step.blockers[0].clone(), Style::default().fg(Color::Yellow)),
            ]));
        }
        if !step.next_steps.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    if is_zh { "下一步: " } else { "Next: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(step.next_steps[0].clone(), Style::default().fg(Color::Cyan)),
            ]));
        }
        if let Some(ref state) = step.scheduler_state {
            let color = match state.as_str() {
                "review_queue" => Color::Red,
                "proposed_followup" => Color::Magenta,
                "pending_coordination" => Color::Yellow,
                _ => Color::DarkGray,
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if is_zh { "工作流: " } else { "Workflow: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(state.clone(), Style::default().fg(color)),
            ]));
        }
    }

    if review_count > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                if is_zh { "提示: " } else { "Hint: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                if is_zh { "需要人工复核" } else { "needs human review" },
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else if followup_count > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                if is_zh { "提示: " } else { "Hint: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                if is_zh { "已生成后续提议" } else { "follow-up proposed" },
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else if pending_count > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                if is_zh { "提示: " } else { "Hint: " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                if is_zh { "待协调" } else { "pending coordination" },
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}
