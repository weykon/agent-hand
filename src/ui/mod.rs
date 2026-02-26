mod app;
mod dialogs;
mod events;
mod input;
mod render;
mod switcher;

pub use app::App;
pub use dialogs::{
    AnnotateDialog, ContextInjectionMethod, CreateGroupDialog, CreateRelationshipDialog,
    DeleteConfirmDialog, DeleteGroupChoice, DeleteGroupDialog, Dialog, ForkDialog, ForkField,
    MoveGroupDialog, NewFromContextDialog, NewSessionDialog, NewSessionField, RenameGroupDialog,
    RenameSessionDialog, SessionEditField, ShareDialog, TagPickerDialog, TagSpec,
};
pub use input::TextInput;
pub use switcher::run_switcher;

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
    Relationships,
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
