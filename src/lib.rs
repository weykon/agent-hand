pub mod cli;
pub mod error;
pub mod mcp;
pub mod session;
pub mod tmux;
pub mod ui;

pub use error::{Error, Result};

/// Version of agent-deck
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
