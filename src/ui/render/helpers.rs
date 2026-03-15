use super::*;


pub(super) fn activity_anim(tick: u64) -> &'static str {
    const FRAMES: [&str; 4] = ["◐", "◓", "◑", "◒"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

pub(super) fn running_anim(tick: u64) -> &'static str {
    // Claude-style small/medium/large dot pulse.
    const FRAMES: [&str; 4] = ["·", "●", "⬤", "●"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

pub(super) fn waiting_anim(tick: u64) -> &'static str {
    // Blink to draw attention: ~1s on, ~0.3s off (tick is 250ms).
    const FRAMES: [&str; 5] = ["!", "!", "!", "!", " "];
    FRAMES[(tick as usize) % FRAMES.len()]
}

#[cfg(feature = "pro")]
pub(super) fn connection_pulse(tick: u64) -> &'static str {
    // Subtle pulse for active connections
    const FRAMES: [&str; 4] = ["◉", "◉", "○", "○"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

/// Parse hex color string (e.g., "#3b82f6") to ratatui Color.
#[cfg(feature = "pro")]
pub(super) fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Minimum visible width for input fields (in characters)
pub(super) const INPUT_MIN_WIDTH: usize = 30;

/// Render a TextInput with cursor visible when active.
/// A subtle background strip marks the editable area so users can see where to type.
pub(super) fn render_text_input(input: &TextInput, active: bool, _base_style: Style) -> Vec<Span<'static>> {
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

#[cfg(feature = "pro")]
pub(super) fn truncate_name(name: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if UnicodeWidthStr::width(name) <= max_width {
        name.to_string()
    } else {
        // Truncate respecting display width (CJK chars = 2 columns)
        let mut w = 0;
        let mut end = 0;
        for (i, ch) in name.char_indices() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if w + cw + 2 > max_width { // +2 for ".."
                break;
            }
            w += cw;
            end = i + ch.len_utf8();
        }
        format!("{}..", &name[..end])
    }
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

// ── Dialog/Block helpers ──────────────────────────────────────────────────

/// Build a bordered block with bilingual title.
pub(super) fn dialog_block<'a>(title_zh: &'a str, title_en: &'a str, is_zh: bool) -> Block<'a> {
    let t = crate::ui::theme::theme();
    Block::default()
        .borders(Borders::ALL)
        .border_style(t.dialog_border_style())
        .title(if is_zh { title_zh } else { title_en })
}

/// Build a bordered block with a single title (no i18n).
pub(super) fn titled_block(title: &str) -> Block<'_> {
    let t = crate::ui::theme::theme();
    Block::default()
        .borders(Borders::ALL)
        .border_style(t.dialog_border_style())
        .title(title)
}

/// Build a plain bordered block with no title.
pub(super) fn plain_block() -> Block<'static> {
    Block::default().borders(Borders::ALL)
}

/// Compute a unicode-aware display width for terminal text.
pub(super) fn display_width(s: &str) -> u16 {
    unicode_width::UnicodeWidthStr::width(s) as u16
}

/// Format room age as a human-readable string (e.g. "5 min ago").
#[cfg(feature = "pro")]
pub(super) fn format_room_age(created_at: &str, is_zh: bool) -> String {
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


