//! Chat types — shared data structures for conversations and messages.

/// Role of a chat message participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    /// Message from the user.
    User,
    /// Response from the assistant / AI.
    Assistant,
    /// System-level message (e.g. context injection).
    System,
}

/// A single message in a conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Who sent this message.
    pub role: ChatRole,
    /// Text content.
    pub content: String,
    /// Unix timestamp when the message was created.
    pub timestamp: f64,
    /// Conversation this message belongs to.
    pub conversation_id: String,
}

/// A multi-turn conversation.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// Unique conversation identifier.
    pub id: String,
    /// Ordered list of messages.
    pub messages: Vec<ChatMessage>,
    /// Optional linked agent-hand session (tmux session name).
    pub target_session: Option<String>,
    /// Unix timestamp when the conversation was created.
    pub created_at: f64,
}

/// Payload forwarded from ActionExecutor when a ChatResponse action is received.
#[derive(Debug, Clone)]
pub struct ChatResponsePayload {
    /// Conversation ID for routing.
    pub conversation_id: String,
    /// Response content (full or chunk for streaming).
    pub content: String,
    /// Whether this is the final chunk.
    pub is_complete: bool,
    /// Optional session context this response relates to.
    pub session_key: Option<String>,
}
