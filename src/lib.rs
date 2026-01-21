pub mod analytics;
pub mod claude;
pub mod cli;
pub mod config;
pub mod error;
pub mod session;
pub mod tmux;
pub mod ui;

pub use error::{Error, Result};

/// Version of agent-hand
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
