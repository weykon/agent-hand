//! Terminal transition animation engine.
//!
//! Provides smooth visual transitions between UI states by interpolating
//! character cells over ~300ms. Includes startup ASCII logo animation.

use std::time::{Duration, Instant};

use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::Color;

// ── Startup Phase ───────────────────────────────────────────────────

/// Tracks which phase of the startup animation we're in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    /// Displaying the ASCII logo with typewriter effect.
    Logo,
    /// Fading out the logo before transitioning to main UI.
    FadeOut,
    /// Startup complete — normal operation.
    Done,
}

// ── ASCII Art ────────────────────────────────────────────────────────

pub const LOGO_ASYMPTAI: &str = r#"
     _                         _     _    ___
    / \   ___ _   _ _ __ ___  | |__ | |_ / _ \  _
   / _ \ / __| | | | '_ ` _ \ | '_ \| __| |_| || |
  / ___ \\__ \ |_| | | | | | || |_) | |_|  _  || |
 /_/   \_\___/\__, |_| |_| |_|| .__/ \__|_| |_||_|
              |___/            |_|
"#;

pub const LOGO_AGENT_HAND: &str = r#"
    _                    _       _   _                 _
   / \   __ _  ___ _ __ | |_    | | | | __ _ _ __   __| |
  / _ \ / _` |/ _ \ '_ \| __|   | |_| |/ _` | '_ \ / _` |
 / ___ \ (_| |  __/ | | | |_    |  _  | (_| | | | | (_| |
/_/   \_\__, |\___|_| |_|\__|   |_| |_|\__,_|_| |_|\__,_|
        |___/
"#;

// ── Transition Effect & Easing ──────────────────────────────────────

/// Visual effect used during frame-to-frame transitions.
#[derive(Debug, Clone, Copy)]
pub enum TransitionEffect {
    /// Characters morph through a density gradient.
    Morph,
    /// Random pixel flip using deterministic hash.
    Dissolve,
    /// Left-to-right wave reveal.
    Cascade,
    /// Linear RGB color interpolation.
    Crossfade,
}

/// Easing function for animation progress.
#[derive(Debug, Clone, Copy)]
pub enum EasingFunction {
    Linear,
    /// Slow start, fast middle, slow end (default).
    EaseInOutCubic,
    EaseOutQuad,
}

impl EasingFunction {
    /// Map linear progress `t` (0..1) to eased progress.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0_f64).powi(3) / 2.0
                }
            }
            Self::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        }
    }
}

// ── Cell Snapshot ────────────────────────────────────────────────────

/// A captured snapshot of a rectangular region of the terminal buffer.
#[derive(Clone)]
pub struct CellSnapshot {
    pub area: Rect,
    cells: Vec<Cell>,
}

impl CellSnapshot {
    /// Capture a rectangular region from a ratatui buffer.
    pub fn capture(buf: &Buffer, area: Rect) -> Self {
        let mut cells = Vec::with_capacity((area.width as usize) * (area.height as usize));
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if x < buf.area.x.saturating_add(buf.area.width)
                    && y < buf.area.y.saturating_add(buf.area.height)
                {
                    cells.push(buf.cell((x, y)).cloned().unwrap_or_default());
                } else {
                    cells.push(Cell::default());
                }
            }
        }
        Self { area, cells }
    }

    /// Get a cell at (x, y) in absolute coordinates.
    fn get(&self, x: u16, y: u16) -> &Cell {
        let col = (x.saturating_sub(self.area.x)) as usize;
        let row = (y.saturating_sub(self.area.y)) as usize;
        let idx = row * (self.area.width as usize) + col;
        self.cells.get(idx).unwrap_or(&DEFAULT_CELL)
    }
}

// Static default cell for out-of-bounds access.
static DEFAULT_CELL: std::sync::LazyLock<Cell> = std::sync::LazyLock::new(Cell::default);

// ── Active Transition ───────────────────────────────────────────────

/// An in-progress transition between two frame snapshots.
pub struct ActiveTransition {
    from: CellSnapshot,
    to: CellSnapshot,
    effect: TransitionEffect,
    easing: EasingFunction,
    started_at: Instant,
    duration: Duration,
}

impl ActiveTransition {
    /// Whether this transition has completed.
    fn is_complete(&self) -> bool {
        self.started_at.elapsed() >= self.duration
    }

    /// Current linear progress (0.0 to 1.0).
    fn progress(&self) -> f64 {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let total = self.duration.as_secs_f64();
        if total <= 0.0 {
            1.0
        } else {
            (elapsed / total).min(1.0)
        }
    }
}

// ── Transition Engine ───────────────────────────────────────────────

/// Manages frame-to-frame transitions with interpolated rendering.
pub struct TransitionEngine {
    active: Option<ActiveTransition>,
    enabled: bool,
    transition_requested: bool,
    last_frame: Option<CellSnapshot>,
}

impl TransitionEngine {
    pub fn new(enabled: bool) -> Self {
        Self {
            active: None,
            enabled,
            transition_requested: false,
            last_frame: None,
        }
    }

    /// Whether an animation is currently playing.
    pub fn is_animating(&self) -> bool {
        self.active.is_some()
    }

    /// Mark that a transition should start on the next draw cycle.
    pub fn request_transition(&mut self) {
        if self.enabled {
            self.transition_requested = true;
        }
    }

    /// Check if a transition was requested (and clear the flag).
    pub fn should_start_transition(&mut self) -> bool {
        if self.transition_requested {
            self.transition_requested = false;
            true
        } else {
            false
        }
    }

    /// Start a transition from the last saved frame to the current buffer content.
    /// If there is no previous frame, no transition is started.
    pub fn start_from_last_frame(&mut self, buf: &Buffer, area: Rect) {
        if !self.enabled {
            return;
        }
        let Some(from) = self.last_frame.take() else {
            return;
        };
        let to = CellSnapshot::capture(buf, area);

        // Cancel any existing transition
        self.active = Some(ActiveTransition {
            from,
            to,
            effect: TransitionEffect::Dissolve,
            easing: EasingFunction::EaseInOutCubic,
            started_at: Instant::now(),
            duration: Duration::from_millis(300),
        });
    }

    /// Save the current buffer as the "last frame" for future transitions.
    pub fn save_frame(&mut self, buf: &Buffer, area: Rect) {
        if self.enabled {
            self.last_frame = Some(CellSnapshot::capture(buf, area));
        }
    }

    /// Apply the current transition interpolation onto the buffer.
    pub fn apply_frame(&self, buf: &mut Buffer) {
        let Some(ref t) = self.active else {
            return;
        };

        let raw_t = t.progress();
        let eased_t = t.easing.apply(raw_t);

        let area = t.to.area;
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if x >= buf.area.x.saturating_add(buf.area.width)
                    || y >= buf.area.y.saturating_add(buf.area.height)
                {
                    continue;
                }
                let from_cell = t.from.get(x, y);
                let to_cell = t.to.get(x, y);
                let interp = interpolate_cell(from_cell, to_cell, eased_t, t.effect, x, y);
                if let Some(cell) = buf.cell_mut((x, y)) {
                    *cell = interp;
                }
            }
        }
    }

    /// Advance the engine: complete transitions that have finished.
    pub fn tick(&mut self) {
        if let Some(ref t) = self.active {
            if t.is_complete() {
                self.active = None;
            }
        }
    }

    /// Cancel any in-progress animation (e.g. on terminal resize).
    pub fn cancel(&mut self) {
        self.active = None;
        self.last_frame = None;
    }

    /// Enable or disable animations.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.active = None;
            self.last_frame = None;
            self.transition_requested = false;
        }
    }
}

// ── Interpolation Helpers ───────────────────────────────────────────

/// Density gradient for character morphing.
const DENSITY_CHARS: &[char] = &[' ', '.', ':', '|', '+', '*', '#', '@'];

/// Interpolate a single cell between two states.
fn interpolate_cell(
    from: &Cell,
    to: &Cell,
    t: f64,
    effect: TransitionEffect,
    x: u16,
    y: u16,
) -> Cell {
    match effect {
        TransitionEffect::Morph => {
            let ch = morph_char(cell_char(from), cell_char(to), t);
            let fg = lerp_color(from.fg, to.fg, t);
            let bg = lerp_color(from.bg, to.bg, t);
            let mut cell = to.clone();
            cell.set_char(ch);
            cell.fg = fg;
            cell.bg = bg;
            cell
        }
        TransitionEffect::Dissolve => {
            if dissolve_reveal(x, y, t) {
                to.clone()
            } else {
                from.clone()
            }
        }
        TransitionEffect::Cascade => {
            // Wave moves left to right. Position determines reveal threshold.
            let wave_pos = t * 1.3; // slightly overshoot so tail finishes
            let cell_threshold = (x as f64) / 80.0; // normalize to ~80 cols
            if wave_pos > cell_threshold {
                to.clone()
            } else {
                from.clone()
            }
        }
        TransitionEffect::Crossfade => {
            let fg = lerp_color(from.fg, to.fg, t);
            let bg = lerp_color(from.bg, to.bg, t);
            let ch = if t < 0.5 {
                cell_char(from)
            } else {
                cell_char(to)
            };
            let mut cell = to.clone();
            cell.set_char(ch);
            cell.fg = fg;
            cell.bg = bg;
            cell
        }
    }
}

/// Get the display character from a cell.
fn cell_char(cell: &Cell) -> char {
    cell.symbol().chars().next().unwrap_or(' ')
}

/// Morph a character through density levels.
fn morph_char(from: char, to: char, t: f64) -> char {
    if from == to {
        return from;
    }
    let from_density = density_index(from);
    let to_density = density_index(to);
    let interp = from_density as f64 + (to_density as f64 - from_density as f64) * t;
    let idx = (interp.round() as usize).min(DENSITY_CHARS.len() - 1);
    DENSITY_CHARS[idx]
}

/// Find density index for a character.
fn density_index(ch: char) -> usize {
    DENSITY_CHARS
        .iter()
        .position(|&c| c == ch)
        .unwrap_or_else(|| {
            // Map unknown chars by visual "weight"
            if ch.is_alphanumeric() {
                6 // heavy like '#'
            } else if ch.is_ascii_punctuation() {
                4 // medium
            } else {
                0 // space-like
            }
        })
}

/// Linear interpolation between two colors.
/// For non-RGB colors, snaps at t=0.5.
fn lerp_color(from: Color, to: Color, t: f64) -> Color {
    if from == to {
        return from;
    }
    match (from, to) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let r = lerp_u8(r1, r2, t);
            let g = lerp_u8(g1, g2, t);
            let b = lerp_u8(b1, b2, t);
            Color::Rgb(r, g, b)
        }
        _ => {
            // Non-RGB: snap at midpoint
            if t < 0.5 {
                from
            } else {
                to
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let result = a as f64 + (b as f64 - a as f64) * t;
    result.round().clamp(0.0, 255.0) as u8
}

/// Deterministic hash-based reveal for dissolve effect.
fn dissolve_reveal(x: u16, y: u16, t: f64) -> bool {
    // Simple hash: each cell has a fixed threshold based on position
    let hash = (x as u32)
        .wrapping_mul(2654435761)
        .wrapping_add((y as u32).wrapping_mul(2246822519));
    let threshold = (hash % 1000) as f64 / 1000.0;
    t > threshold
}

// ── Startup Rendering ───────────────────────────────────────────────

/// Compute how many characters of the logo should be revealed.
/// Returns number of chars to show (typewriter effect).
pub fn logo_chars_revealed(elapsed_ms: u64, total_chars: usize) -> usize {
    // Reveal over 1200ms of the 1500ms logo phase
    let reveal_duration = 1200u64;
    if elapsed_ms >= reveal_duration {
        total_chars
    } else {
        let progress = elapsed_ms as f64 / reveal_duration as f64;
        // Ease-out for natural typing feel
        let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
        (eased * total_chars as f64).ceil() as usize
    }
}

/// Compute fadeout alpha (1.0 = fully visible, 0.0 = invisible).
pub fn fadeout_alpha(elapsed_ms: u64) -> f64 {
    // FadeOut phase is 1500..2000ms → 500ms fade
    if elapsed_ms < 1500 {
        1.0
    } else if elapsed_ms >= 2000 {
        0.0
    } else {
        let t = (elapsed_ms - 1500) as f64 / 500.0;
        1.0 - t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_bounds() {
        for ease in [
            EasingFunction::Linear,
            EasingFunction::EaseInOutCubic,
            EasingFunction::EaseOutQuad,
        ] {
            assert!((ease.apply(0.0) - 0.0).abs() < 1e-10);
            assert!((ease.apply(1.0) - 1.0).abs() < 1e-10);
            // Monotonic
            let mut prev = 0.0;
            for i in 0..=100 {
                let t = i as f64 / 100.0;
                let v = ease.apply(t);
                assert!(v >= prev - 1e-10, "Easing not monotonic at t={t}");
                prev = v;
            }
        }
    }

    #[test]
    fn dissolve_coverage() {
        // At t=1.0, all cells should be revealed
        for y in 0..50u16 {
            for x in 0..80u16 {
                assert!(dissolve_reveal(x, y, 1.0));
            }
        }
        // At t=0.0, no cells revealed
        let mut revealed = 0;
        for y in 0..50u16 {
            for x in 0..80u16 {
                if dissolve_reveal(x, y, 0.0) {
                    revealed += 1;
                }
            }
        }
        // Should be very few (< 1%)
        assert!(revealed < 40, "Too many revealed at t=0: {revealed}");
    }

    #[test]
    fn morph_identity() {
        assert_eq!(morph_char('A', 'A', 0.5), 'A');
    }

    #[test]
    fn logo_reveal_progress() {
        assert_eq!(logo_chars_revealed(0, 100), 0);
        assert_eq!(logo_chars_revealed(1300, 100), 100);
        let mid = logo_chars_revealed(600, 100);
        assert!(mid > 20 && mid < 90, "Mid reveal should be partial: {mid}");
    }

    #[test]
    fn lerp_color_rgb() {
        let c = lerp_color(Color::Rgb(0, 0, 0), Color::Rgb(100, 200, 50), 0.5);
        assert_eq!(c, Color::Rgb(50, 100, 25));
    }

    #[test]
    fn lerp_color_non_rgb_snaps() {
        let c = lerp_color(Color::Red, Color::Blue, 0.3);
        assert_eq!(c, Color::Red);
        let c = lerp_color(Color::Red, Color::Blue, 0.7);
        assert_eq!(c, Color::Blue);
    }
}
