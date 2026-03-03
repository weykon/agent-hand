mod app;
mod dialogs;
mod events;
mod input;
mod render;
mod switcher;

pub use app::App;
pub use dialogs::{
    CreateGroupDialog,
    DeleteConfirmDialog, DeleteGroupChoice, DeleteGroupDialog, Dialog,
    ForkDialog, ForkField, MoveGroupDialog, NewSessionDialog,
    NewSessionField, RenameGroupDialog, RenameSessionDialog, SessionEditField,
    SettingsDialog, SettingsField, SettingsTab,
    TagPickerDialog, TagSpec,
};

#[cfg(feature = "pro")]
pub use dialogs::{
    AnnotateDialog, ContextInjectionMethod, CreateRelationshipDialog,
    CreateRelationshipField, NewFromContextDialog, ShareDialog,
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
    #[cfg(feature = "pro")]
    Relationships,
    /// Viewing a shared terminal session via relay (Pro only).
    #[cfg(feature = "pro")]
    ViewerMode,
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
