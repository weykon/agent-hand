pub mod config;
pub mod manager;

#[cfg(unix)]
pub mod pool;

#[cfg(not(unix))]
#[path = "pool_stub.rs"]
pub mod pool;

pub use config::MCPConfig;
pub use manager::MCPManager;
pub use pool::{pooled_mcp_config, MCPPool};
