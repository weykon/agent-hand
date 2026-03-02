pub mod ai;
pub mod analytics;
pub mod auth;
pub mod claude;
pub mod cli;
pub mod config;
pub mod error;
pub mod session;
pub mod sharing;
pub mod tmux;
pub mod ui;
pub mod update;

#[cfg(feature = "pro")]
#[path = "../pro/src/mod.rs"]
pub mod pro;

pub use error::{Error, Result};

/// Version of agent-hand
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
