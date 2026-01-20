mod cache;
mod detector;
mod manager;
mod session;

pub use cache::SessionCache;
pub use detector::{set_status_detection_config, PromptDetector, Tool};
pub use manager::TmuxManager;
pub use session::{SessionStatus, TmuxSession};

pub const SESSION_PREFIX: &str = "agentdeck_rs_";
