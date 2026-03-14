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

mod helpers;
mod sessions;
mod dialogs;
#[cfg(feature = "pro")]
#[path = "../../../pro/src/ui/render/viewer.rs"]
mod viewer;
#[cfg(test)]
mod tests;

// Pro/Max render functions — files in pro/ loaded as submodules
#[cfg(feature = "pro")]
#[path = "../../../pro/src/ui/render/dialogs.rs"]
mod dialogs_pro;
#[cfg(feature = "max")]
#[path = "../../../pro/src/ui/render/dialogs_max.rs"]
mod dialogs_max;

use helpers::*;
use sessions::*;
use dialogs::*;
#[cfg(feature = "pro")]
use dialogs_pro::*;
#[cfg(feature = "max")]
use dialogs_max::*;
#[cfg(feature = "pro")]
use viewer::*;

/// Main render function
pub fn draw(f: &mut Frame, app: &App) {
    // Startup splash screen
    if app.state() == crate::ui::AppState::Startup {
        render_startup(f, f.area(), app.startup_phase(), app.startup_elapsed_ms());
        return;
    }

    // In ViewerMode, use full screen for PTY content (no title/status bars)
    // This ensures the viewer displays exactly what the host terminal shows
    #[cfg(feature = "pro")]
    if app.state() == crate::ui::AppState::ViewerMode {
        render_viewer_mode(f, f.area(), app);

        // Still render overlays if needed
        if app.help_visible() {
            render_help_modal(f, f.area(), app.language());
        }
        if app.state() == crate::ui::AppState::Dialog {
            render_dialog(f, f.area(), app);
        }
        render_toast_notifications(f, f.area(), app);
        return;
    }

    // Normal layout for other modes (with title and status bars)
    let has_info = app.info_bar_message().is_some();
    let chunks = if has_info {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(1), // Info bar
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status bar
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status bar
            ])
            .split(f.area())
    };

    // Render title
    render_title(f, chunks[0], app.language());

    let (content_idx, status_idx) = if has_info {
        render_info_bar(f, chunks[1], app);
        (2, 3)
    } else {
        (1, 2)
    };

    // Always render main content (dashboard stays visible behind modal)
    let content_area = chunks[content_idx];
    if app.chat_visible() {
        // Split content: main (65%) | chat (35%)
        let chat_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(content_area);

        #[cfg(feature = "pro")]
        {
            if app.state() == crate::ui::AppState::Relationships {
                render_relationships(f, chat_split[0], app);
            } else {
                render_main(f, chat_split[0], app);
            }
        }
        #[cfg(not(feature = "pro"))]
        render_main(f, chat_split[0], app);

        render_chat_panel(f, chat_split[1], app);
    } else {
        #[cfg(feature = "pro")]
        {
            if app.state() == crate::ui::AppState::Relationships {
                render_relationships(f, content_area, app);
            } else {
                render_main(f, content_area, app);
            }
        }
        #[cfg(not(feature = "pro"))]
        render_main(f, content_area, app);
    }

    // Render status bar
    render_status_bar(f, chunks[status_idx], app);

    // Keyboard hints overlay (bottom-right of content area)
    render_hints_overlay(f, chunks[content_idx], app);

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

    // AI summary overlay popup (Max tier)
    #[cfg(feature = "max")]
    if app.max.show_ai_summary_overlay {
        render_ai_summary_overlay(f, f.area(), app);
    }

    // AI diagram overlay popup (Max tier)
    #[cfg(feature = "max")]
    if app.max.show_ai_diagram_overlay {
        render_ai_diagram_overlay(f, f.area(), app);
    }

    // Behavior analysis overlay popup (Max tier)
    #[cfg(feature = "max")]
    if app.max.show_behavior_overlay {
        render_behavior_overlay(f, f.area(), app);
    }

    // Onboarding welcome message
    if app.show_onboarding() {
        render_onboarding_welcome(f, f.area(), app.language());
    }
}

/// Render the info bar (version update or tier mismatch hint)
fn render_info_bar(f: &mut Frame, area: Rect, app: &App) {
    if let Some((msg, color)) = app.info_bar_message() {
        let bar = Paragraph::new(Span::styled(
            msg.clone(),
            Style::default()
                .fg(Color::Black)
                .bg(*color)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .style(Style::default().bg(*color));
        f.render_widget(bar, area);
    }
}

/// Render title bar
fn render_title(f: &mut Frame, area: Rect, lang: crate::i18n::Language) {
    use crate::i18n::{Translate, Language};

    let title_text = match lang {
        Language::Chinese => "🦀 Agent Hand 智能助手",
        Language::English => "🦀 Agent Hand",
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

    // Pro: canvas fills the right side (no preview panel)
    // Free: preview only
    #[cfg(feature = "pro")]
    {
        let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
        let show_edge_detail = app.canvas_focused()
            && app.canvas_state().selected_edge_relationship_id().is_some();
        // Show the projection detail panel only in User view
        // (in Agent view the tab content IS the detail).
        let show_projection_detail = app.canvas_focused()
            && !app.canvas_state().is_projection_view();

        if show_edge_detail {
            // Split right column: canvas + relationship detail panel
            let right_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(6), Constraint::Length(12)])
                .split(cols[1]);
            crate::ui::canvas::render::render_canvas(f, right_split[0], app.canvas_state(), app.canvas_focused(), is_zh);
            crate::ui::render::sessions::render_relationship_detail(f, right_split[1], app);
        } else if show_projection_detail {
            let right_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(6), Constraint::Length(12)])
                .split(cols[1]);
            crate::ui::canvas::render::render_canvas(f, right_split[0], app.canvas_state(), app.canvas_focused(), is_zh);
            crate::ui::render::sessions::render_canvas_projection_detail(f, right_split[1], app);
        } else {
            crate::ui::canvas::render::render_canvas(f, cols[1], app.canvas_state(), app.canvas_focused(), is_zh);
        }
    }
    #[cfg(not(feature = "pro"))]
    {
        crate::ui::render::sessions::render_preview(f, cols[1], app);
    }
}

fn render_chat_panel(f: &mut Frame, area: Rect, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let is_focused = app.state() == crate::ui::AppState::Chat;

    // Split: message history (top) + input line (bottom)
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    // -- Message history --
    let messages = app.chat_messages();
    let mut lines: Vec<Line> = Vec::new();
    for msg in &messages {
        let (prefix, color) = match msg.role {
            crate::chat::ChatRole::User => {
                let label = if is_zh { "你: " } else { "You: " };
                (label, Color::Cyan)
            }
            crate::chat::ChatRole::Assistant => ("🤖 ", Color::Green),
            crate::chat::ChatRole::System => {
                let label = if is_zh { "系统: " } else { "Sys: " };
                (label, Color::DarkGray)
            }
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::raw(&msg.content),
        ]));
    }

    if lines.is_empty() {
        let hint = if is_zh {
            "输入消息开始聊天..."
        } else {
            "Type a message to start chatting..."
        };
        lines.push(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        )));
    }

    let border_color = if is_focused { Color::Cyan } else { Color::DarkGray };
    let title = if is_zh { " 聊天 " } else { " Chat " };

    // Apply scroll: lines are rendered bottom-up (newest at bottom)
    let visible_height = chat_layout[0].height.saturating_sub(2) as usize; // minus borders
    let total = lines.len();
    let scroll_offset = app.chat_scroll() as usize;
    let start = if total > visible_height + scroll_offset {
        total - visible_height - scroll_offset
    } else {
        0
    };
    let end = if total > scroll_offset {
        total - scroll_offset
    } else {
        total
    };
    let visible_lines: Vec<Line> = lines[start..end.min(total)].to_vec();

    let messages_widget = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(messages_widget, chat_layout[0]);

    // -- Input line --
    let input_text = app.chat_input();
    let cursor_char = if is_focused { "▎" } else { "" };
    let input_display = format!("{}{}", input_text, cursor_char);

    let input_widget = Paragraph::new(input_display)
        .block(
            Block::default()
                .title(if is_zh { " 输入 " } else { " Input " })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(input_widget, chat_layout[1]);
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

    let mut spans: Vec<Span> = Vec::new();

    // Activity tracker: show spinner for active async operations
    if let Some(activity) = app.activity().current() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("{} {}", activity_anim(app.tick_count()), activity.message),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
    }

    spans.extend([
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
    ]);

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

    if app.canvas_focused() {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled("u", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(if is_zh { ":撤销" } else { ":undo" }));
    } else if let Some(session) = app.selected_session() {
        if session.cli_session_id().is_some() {
            spans.push(Span::raw("  |  "));
            spans.push(Span::styled("u", Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(if is_zh {
                ":恢复已停止会话"
            } else {
                ":resume stopped CLI"
            }));
        }
    }

    if app.state() == crate::ui::AppState::Search {
        spans.push(Span::styled(
            if is_zh { "搜索: " } else { "Search: " },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(app.search_query().to_string()));
        spans.push(Span::raw(format!(" ({})", app.search_matches())));
        spans.push(Span::raw("  "));
    }

    // User account badge
    spans.push(Span::raw("  |  "));
    if let Some(token) = app.auth_token() {
        if token.is_max() {
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

/// Collect keyboard hints as structured (key, label, color) tuples for the overlay.
/// Context-aware: shows different hints based on current focus (canvas, active panel, tree, etc.)
fn collect_overlay_hints(app: &App) -> Vec<(&'static str, &'static str, Color)> {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let mut hints: Vec<(&str, &str, Color)> = Vec::new();

    // Priority: chat > canvas > active panel > viewer panel > relationships > tree
    if app.state() == crate::ui::AppState::Chat {
        hints.extend([
            ("Enter", if is_zh { "发送" } else { "send" }, Color::Green),
            ("\u{2191}/\u{2193}", if is_zh { "滚动" } else { "scroll" }, Color::Cyan),
            ("Esc", if is_zh { "关闭" } else { "close" }, Color::Yellow),
        ]);
        return hints;
    }

    #[cfg(feature = "pro")]
    if app.canvas_focused() {
        collect_canvas_hints(&mut hints, app);
        return hints;
    }

    #[cfg(feature = "pro")]
    if app.active_panel_focused() {
        hints.extend([
            ("j/k", if is_zh { "上下" } else { "nav" }, Color::Cyan),
            ("Enter", if is_zh { "附加" } else { "attach" }, Color::Green),
            ("\u{2192}", if is_zh { "跳转" } else { "jump" }, Color::Cyan),
            ("Esc", if is_zh { "返回" } else { "back" }, Color::Yellow),
        ]);
        return hints;
    }

    #[cfg(feature = "pro")]
    if app.pro.viewer_panel_focused {
        hints.extend([
            ("j/k", if is_zh { "上下" } else { "nav" }, Color::Cyan),
            ("Enter", if is_zh { "附加" } else { "attach" }, Color::Green),
            ("d", if is_zh { "断开" } else { "detach" }, Color::Red),
            ("Esc", if is_zh { "返回" } else { "back" }, Color::Yellow),
        ]);
        return hints;
    }

    // Context-sensitive item hints (tree / relationships)
    #[cfg(feature = "pro")]
    {
        if app.state() == crate::ui::AppState::Relationships {
            hints.extend([
                ("n", if is_zh { "新建" } else { "new" }, Color::Cyan),
                ("d", if is_zh { "删除" } else { "del" }, Color::Red),
                ("c", if is_zh { "捕获" } else { "capture" }, Color::Cyan),
                ("a", if is_zh { "注释" } else { "annotate" }, Color::Cyan),
                ("^N", if is_zh { "从上下文" } else { "from-ctx" }, Color::Cyan),
                ("Esc", if is_zh { "返回" } else { "back" }, Color::Yellow),
            ]);
        } else {
            collect_item_selection_hints(&mut hints, app);
        }
    }
    #[cfg(not(feature = "pro"))]
    collect_item_selection_hints(&mut hints, app);

    // Global hints
    hints.push(("/", if is_zh { "搜索" } else { "search" }, Color::Cyan));
    #[cfg(feature = "pro")]
    hints.push(("^E", if is_zh { "关系" } else { "rels" }, Color::Cyan));
    #[cfg(feature = "pro")]
    hints.push(("p", if is_zh { "画布" } else { "canvas" }, Color::Cyan));
    #[cfg(not(feature = "pro"))]
    hints.push(("p", if is_zh { "预览" } else { "preview" }, Color::Cyan));
    hints.push(("^T", if is_zh { "聊天" } else { "chat" }, Color::Cyan));
    hints.push(("?", if is_zh { "帮助" } else { "help" }, Color::Magenta));
    hints.push(("q", if is_zh { "退出" } else { "quit" }, Color::Red));

    // Tab hint (Pro with active sessions)
    #[cfg(feature = "pro")]
    {
        let is_pro = app.auth_token().map_or(false, |t| t.is_pro());
        let has_active = !app.active_sessions().is_empty();
        if is_pro && has_active {
            hints.push(("Tab", if is_zh { "活跃" } else { "active" }, Color::Yellow));
        }
    }

    hints
}

/// Collect canvas-specific keyboard hints.
#[cfg(feature = "pro")]
fn collect_canvas_hints(hints: &mut Vec<(&'static str, &'static str, Color)>, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    let canvas = app.canvas_state();

    if canvas.is_editing() {
        hints.extend([
            ("Enter", if is_zh { "保存" } else { "save" }, Color::Green),
            ("Esc", if is_zh { "取消" } else { "cancel" }, Color::Yellow),
        ]);
        return;
    }
    if canvas.adding_node {
        hints.extend([
            ("1", "Process", Color::Cyan),
            ("2", "Decision", Color::Yellow),
            ("3", "Start", Color::Green),
            ("4", "End", Color::Red),
            ("5", "Note", Color::DarkGray),
            ("Esc", if is_zh { "取消" } else { "cancel" }, Color::Yellow),
        ]);
        return;
    }
    if canvas.in_connect_mode() {
        hints.extend([
            ("Enter", if is_zh { "连接" } else { "connect" }, Color::Green),
            ("Esc", if is_zh { "取消" } else { "cancel" }, Color::Yellow),
        ]);
        return;
    }

    // Normal canvas mode
    hints.extend([
        ("e", if is_zh { "编辑" } else { "edit" }, Color::Cyan),
        ("n", if is_zh { "新节点" } else { "node+" }, Color::Cyan),
        ("d", if is_zh { "删除" } else { "del" }, Color::Red),
        ("c", if is_zh { "连线" } else { "connect" }, Color::Cyan),
        ("Spc", if is_zh { "拖动" } else { "drag" }, Color::Cyan),
        ("z", if is_zh { "居中" } else { "center" }, Color::Cyan),
        ("^hj", if is_zh { "平移" } else { "pan" }, Color::DarkGray),
        ("u", if is_zh { "撤销" } else { "undo" }, Color::Yellow),
        ("^r", if is_zh { "重做" } else { "redo" }, Color::Yellow),
        ("m", if is_zh { "模式" } else { "mode" }, Color::DarkGray),
        ("Esc", if is_zh { "返回" } else { "back" }, Color::Yellow),
    ]);
}

/// Collect item-selection hints based on tree selection (for overlay).
fn collect_item_selection_hints(hints: &mut Vec<(&'static str, &'static str, Color)>, app: &App) {
    let is_zh = matches!(app.language(), crate::i18n::Language::Chinese);
    match app.selected_item() {
        Some(TreeItem::Group { .. }) => {
            hints.extend([
                ("Enter", if is_zh { "切换" } else { "toggle" }, Color::Cyan),
                ("r", if is_zh { "重命名" } else { "rename" }, Color::Yellow),
                ("d", if is_zh { "删除" } else { "del" }, Color::Cyan),
                ("g", if is_zh { "新分组" } else { "group+" }, Color::Cyan),
                ("n", if is_zh { "新建" } else { "new" }, Color::Cyan),
            ]);
        }
        Some(TreeItem::Session { .. } | TreeItem::Relationship { .. }) => {
            hints.extend([
                ("n", if is_zh { "新建" } else { "new" }, Color::Cyan),
                ("g", if is_zh { "新分组" } else { "group+" }, Color::Cyan),
                ("r", if is_zh { "重命名" } else { "rename" }, Color::Yellow),
                ("R", if is_zh { "重启" } else { "restart" }, Color::Yellow),
                ("d", if is_zh { "删除" } else { "del" }, Color::Cyan),
                ("f", if is_zh { "复制" } else { "fork" }, Color::Cyan),
                ("m", if is_zh { "移动" } else { "move" }, Color::Cyan),
                ("b", if is_zh { "置顶" } else { "boost" }, Color::Cyan),
            ]);
            #[cfg(feature = "pro")]
            hints.push(("a", if is_zh { "+画布" } else { "+canvas" }, Color::Green));
            #[cfg(feature = "max")]
            hints.push(("A", "AI", Color::Magenta));
        }
        _ => {
            hints.extend([
                ("n", if is_zh { "新建" } else { "new" }, Color::Cyan),
                ("g", if is_zh { "新分组" } else { "group+" }, Color::Cyan),
            ]);
        }
    }
}

/// Render keyboard hints as a floating overlay in the bottom-right of the content area.
fn render_hints_overlay(f: &mut Frame, content_area: Rect, app: &App) {
    let hints = collect_overlay_hints(app);
    if hints.is_empty() {
        return;
    }

    // Layout: 2-column grid.  Each entry is "key:label" — estimate column width.
    let col_w: u16 = 14;
    let panel_w = (col_w * 2 + 3).min(content_area.width.saturating_sub(2)); // 2 cols + border + padding
    let rows = ((hints.len() + 1) / 2) as u16;
    let panel_h = rows + 2; // +2 for top/bottom border

    if panel_h > content_area.height || panel_w > content_area.width {
        return; // terminal too small
    }

    let x = content_area.right().saturating_sub(panel_w).saturating_sub(1);
    let y = content_area.bottom().saturating_sub(panel_h);

    let panel_rect = Rect::new(x, y, panel_w, panel_h);

    // Build 2-column lines
    let mut lines: Vec<Line> = Vec::new();
    for chunk in hints.chunks(2) {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            chunk[0].0.to_string(),
            Style::default().fg(chunk[0].2),
        ));
        spans.push(Span::styled(
            format!(":{:<10}", chunk[0].1),
            Style::default().fg(Color::DarkGray),
        ));
        if chunk.len() > 1 {
            spans.push(Span::styled(
                chunk[1].0.to_string(),
                Style::default().fg(chunk[1].2),
            ));
            spans.push(Span::styled(
                format!(":{}", chunk[1].1),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(spans));
    }

    f.render_widget(Clear, panel_rect);
    let panel = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 80))),
    );
    f.render_widget(panel, panel_rect);
}

/// Render toast notifications as an overlay in the top-right corner.
#[cfg(feature = "pro")]
fn render_toast_notifications(f: &mut Frame, area: Rect, app: &App) {
    let toasts = &app.pro.toast_notifications;
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

/// Render the startup splash screen with ASCII logo and typewriter animation.
fn render_startup(
    f: &mut Frame,
    area: Rect,
    phase: crate::ui::transition::StartupPhase,
    elapsed_ms: u64,
) {
    use crate::ui::transition::{
        fadeout_alpha, logo_chars_revealed, StartupPhase, LOGO_AGENT_HAND, LOGO_ASYMPTAI,
    };

    // Clear background
    f.render_widget(Clear, area);

    // Combine both logos
    let combined = format!("{}\n{}", LOGO_ASYMPTAI.trim(), LOGO_AGENT_HAND.trim());
    let logo_lines: Vec<&str> = combined.lines().collect();
    let logo_height = logo_lines.len() as u16;
    let logo_width = logo_lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

    // Skip logo if terminal is too small
    if area.width < 40 || area.height < logo_height + 4 {
        // Small terminal: just show a compact version
        let text = "AsymptAI · Agent-Hand";
        let x = area.x + area.width.saturating_sub(text.len() as u16) / 2;
        let y = area.y + area.height / 2;
        if y < area.bottom() {
            let alpha = match phase {
                StartupPhase::FadeOut => fadeout_alpha(elapsed_ms),
                StartupPhase::Done => return,
                _ => 1.0,
            };
            let grey = (alpha * 255.0) as u8;
            let style = Style::default().fg(Color::Rgb(grey, grey, grey));
            let para = Paragraph::new(text).style(style);
            f.render_widget(para, Rect::new(x, y, text.len() as u16, 1));
        }
        return;
    }

    // Center the logo
    let start_x = area.x + area.width.saturating_sub(logo_width) / 2;
    let start_y = area.y + area.height.saturating_sub(logo_height + 2) / 2;

    // Count total displayable characters
    let total_chars: usize = logo_lines.iter().map(|l| l.len()).sum();
    let revealed = match phase {
        StartupPhase::Logo => logo_chars_revealed(elapsed_ms, total_chars),
        StartupPhase::FadeOut | StartupPhase::Done => total_chars,
    };

    // Fade alpha for fade-out phase
    let alpha = match phase {
        StartupPhase::FadeOut => fadeout_alpha(elapsed_ms),
        StartupPhase::Done => return,
        _ => 1.0,
    };

    // Render with typewriter effect + color
    let mut chars_shown = 0usize;
    for (row, line) in logo_lines.iter().enumerate() {
        let y = start_y + row as u16;
        if y >= area.bottom() {
            break;
        }

        let mut spans: Vec<Span> = Vec::new();
        for ch in line.chars() {
            if chars_shown < revealed {
                // Gradient: cyan at top → blue at bottom
                let row_ratio = row as f64 / logo_height.max(1) as f64;
                let r = (0.0 + 30.0 * row_ratio) as u8;
                let g = (220.0 - 100.0 * row_ratio) as u8;
                let b = (255.0 - 55.0 * row_ratio) as u8;

                // Apply fadeout alpha
                let r = (r as f64 * alpha) as u8;
                let g = (g as f64 * alpha) as u8;
                let b = (b as f64 * alpha) as u8;

                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            }
            // Don't render unrevealed chars (leave blank)
            chars_shown += 1;
        }

        if !spans.is_empty() {
            let para = Paragraph::new(Line::from(spans));
            let line_width = line.len().min(area.width as usize) as u16;
            f.render_widget(para, Rect::new(start_x, y, line_width, 1));
        }
    }

    // Version tag + signature below logo
    if chars_shown >= total_chars {
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        let version_len = version.len() as u16;
        let vx = area.x + area.width.saturating_sub(version_len) / 2;
        let vy = start_y + logo_height + 1;
        if vy < area.bottom() {
            let grey = (alpha * 100.0) as u8;
            let style = Style::default().fg(Color::Rgb(grey, grey, grey));
            let para = Paragraph::new(version).style(style);
            f.render_widget(para, Rect::new(vx, vy, version_len, 1));
        }

        // Signature line
        let sig = "design by weykon";
        let sx = area.x + area.width.saturating_sub(sig.len() as u16) / 2;
        let sy = start_y + logo_height + 3;
        if sy < area.bottom() {
            // Subtle warm tone for the signature
            let r = (180.0 * alpha) as u8;
            let g = (140.0 * alpha) as u8;
            let b = (100.0 * alpha) as u8;
            let style = Style::default()
                .fg(Color::Rgb(r, g, b))
                .add_modifier(Modifier::ITALIC);
            let para = Paragraph::new(sig).style(style).alignment(Alignment::Center);
            f.render_widget(para, Rect::new(sx, sy, sig.len() as u16, 1));
        }
    }
}
