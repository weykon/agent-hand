mod groups;
mod instance;
mod storage;

pub use groups::{GroupData, GroupTree};
pub use instance::{Instance, Status};
pub use storage::{Storage, StorageData};

/// Default profile name
pub const DEFAULT_PROFILE: &str = "default";
