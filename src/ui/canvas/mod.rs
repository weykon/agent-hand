//! Canvas workflow editor.
//!
//! When the `pro` feature is enabled, the full implementation comes from `crate::pro::canvas`.
//! Otherwise, stub types and no-op methods are provided so the free version compiles.

// ── Pro path: re-export everything from pro/src/canvas/ ─────────────────────
#[cfg(feature = "pro")]
pub use crate::pro::canvas::*;

// Re-export submodules from pro when pro is enabled
#[cfg(feature = "pro")]
pub mod render {
    pub use crate::pro::canvas::render::*;
}
#[cfg(feature = "pro")]
pub mod input {
    pub use crate::pro::canvas::input::*;
}
#[cfg(feature = "pro")]
pub mod socket {
    pub use crate::pro::canvas::socket::*;
}

// ── Free path: stub types + no-op methods ───────────────────────────────────
#[cfg(not(feature = "pro"))]
mod free_stub;
#[cfg(not(feature = "pro"))]
pub use free_stub::*;
#[cfg(not(feature = "pro"))]
pub mod socket;
#[cfg(not(feature = "pro"))]
pub mod render {
    /// No-op canvas render for free version.
    pub fn render_canvas(
        _f: &mut ratatui::Frame,
        _area: ratatui::layout::Rect,
        _state: &super::CanvasState,
        _focused: bool,
        _is_zh: bool,
    ) {
    }
}
#[cfg(not(feature = "pro"))]
pub mod input {
    /// No-op canvas input handler for free version.
    pub fn handle_canvas_input(
        _state: &mut super::CanvasState,
        _key: crossterm::event::KeyCode,
        _modifiers: crossterm::event::KeyModifiers,
        _cols: u16,
        _rows: u16,
    ) -> bool {
        false
    }
}
