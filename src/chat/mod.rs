//! Chat module — shared backend for TUI chat panel and CLI REPL.
//!
//! Provides ChatService for managing conversations, sending user messages
//! as HookEvents, and receiving ChatResponse actions from the agent framework.

pub mod service;
pub mod types;

pub use service::{ChatError, ChatService};
pub use types::{ChatMessage, ChatResponsePayload, ChatRole, Conversation};
