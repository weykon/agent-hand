pub mod agent;
pub mod ai;
pub mod analytics;
pub mod auth;
pub mod chat;
pub mod claude;
pub mod device;
pub mod cli;
pub mod config;
pub mod control;
pub mod hooks;
pub mod error;
pub mod i18n;
pub mod session;
pub mod sharing;
#[cfg(feature = "pro")]
pub mod skills;
pub mod tmux;
pub mod ui;
pub mod update;
#[cfg(feature = "max")]
pub mod ws;

pub mod notification;

#[cfg(feature = "pro")]
#[path = "../pro/src/mod.rs"]
pub mod pro;

pub use error::{Error, Result};

/// Version of agent-hand
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
