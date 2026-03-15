mod cache;
mod detector;
mod manager;
pub mod ptmx;
pub mod resume_adapter;
mod session;
pub mod session_id_scanner;

pub use cache::SessionCache;
pub use detector::{set_status_detection_config, PromptDetector, Tool};
pub use manager::TmuxManager;
pub use session::{SessionStatus, TmuxSession};

pub const SESSION_PREFIX: &str = "agentdeck_rs_";

/// Single source of truth for the tmux server socket name.
/// All tmux commands must use `-L <server_name>` with this value.
pub fn server_name_for_profile(profile: &str) -> String {
    format!("agenthand_{}", profile)
}

/// Build a `tokio::process::Command` pre-configured with `-L <server>`.
pub fn async_tmux_cmd(profile: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("tmux");
    cmd.args(["-L", &server_name_for_profile(profile)]);
    cmd
}

/// Build a `std::process::Command` pre-configured with `-L <server>`.
/// Use in `spawn_blocking` or other sync contexts.
pub fn sync_tmux_cmd(profile: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("tmux");
    cmd.args(["-L", &server_name_for_profile(profile)]);
    cmd
}
