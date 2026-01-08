pub mod analytics;
pub mod cli;
pub mod config;
pub mod error;
#[cfg(feature = "input-logging")]
pub mod log_rotate;
pub mod mcp;
pub mod session;
pub mod tmux;
pub mod ui;

pub use error::{Error, Result};

/// Version of agent-hand
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
