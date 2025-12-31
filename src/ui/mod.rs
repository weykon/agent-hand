mod app;
mod dialogs;
mod events;
mod render;

pub use app::App;
pub use dialogs::{
    DeleteConfirmDialog, Dialog, ForkDialog, ForkField, MCPColumn, MCPDialog, NewSessionDialog,
    NewSessionField, NewSessionTool,
};

use crossterm::event::{KeyCode, KeyModifiers};

/// UI events
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Key(KeyCode, KeyModifiers),
    Tick,
    Resize(u16, u16),
    Quit,
}

/// Application state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Normal,
    Search,
    Dialog,
    Help,
}

#[derive(Debug, Clone)]
pub enum TreeItem {
    Group {
        path: String,
        name: String,
        depth: usize,
    },
    Session {
        id: String,
        depth: usize,
    },
}
