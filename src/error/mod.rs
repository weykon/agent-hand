use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Tmux error: {0}")]
    Tmux(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Profile error: {0}")]
    Profile(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn tmux(msg: impl Into<String>) -> Self {
        Self::Tmux(msg.into())
    }

    pub fn mcp(msg: impl Into<String>) -> Self {
        Self::Mcp(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }

    pub fn profile(msg: impl Into<String>) -> Self {
        Self::Profile(msg.into())
    }
}
