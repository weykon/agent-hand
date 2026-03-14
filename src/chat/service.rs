//! ChatService — shared backend for TUI chat panel and CLI REPL.
//!
//! Manages conversations, sends UserChat events into the hook broadcast channel,
//! and receives ChatResponse actions back via a dedicated mpsc channel.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc};

use crate::hooks::{HookEvent, HookEventKind};

use super::types::{ChatMessage, ChatResponsePayload, ChatRole, Conversation};

/// Shared chat backend used by both TUI and CLI consumers.
pub struct ChatService {
    /// Active conversations keyed by conversation ID.
    conversations: HashMap<String, Conversation>,
    /// Send UserChat events into the hook broadcast channel.
    event_tx: broadcast::Sender<HookEvent>,
    /// Receive ChatResponse actions forwarded by ActionExecutor.
    response_rx: mpsc::UnboundedReceiver<ChatResponsePayload>,
    /// Counter for generating unique conversation IDs.
    next_id: u64,
}

impl ChatService {
    /// Create a new ChatService wired to the hook broadcast and response channels.
    pub fn new(
        event_tx: broadcast::Sender<HookEvent>,
        response_rx: mpsc::UnboundedReceiver<ChatResponsePayload>,
    ) -> Self {
        Self {
            conversations: HashMap::new(),
            event_tx,
            response_rx,
            next_id: 1,
        }
    }

    /// Create a new conversation, optionally linked to an agent-hand session.
    /// Returns the new conversation ID.
    pub fn create_conversation(&mut self, target_session: Option<String>) -> String {
        let id = format!("conv-{}", self.next_id);
        self.next_id += 1;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let conversation = Conversation {
            id: id.clone(),
            messages: Vec::new(),
            target_session,
            created_at: now,
        };
        self.conversations.insert(id.clone(), conversation);
        id
    }

    /// Send a user message into a conversation.
    ///
    /// Records the message in conversation history and publishes a UserChat
    /// HookEvent into the broadcast channel for the agent framework to process.
    pub fn send_message(
        &mut self,
        conversation_id: &str,
        message: &str,
        target_session: Option<&str>,
    ) -> Result<(), ChatError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        // Ensure conversation exists
        let conv = self
            .conversations
            .get_mut(conversation_id)
            .ok_or(ChatError::ConversationNotFound)?;

        // Record user message
        conv.messages.push(ChatMessage {
            role: ChatRole::User,
            content: message.to_string(),
            timestamp: now,
            conversation_id: conversation_id.to_string(),
        });

        // Resolve tmux session: prefer explicit target, fall back to conversation default
        let tmux_session = target_session
            .map(|s| s.to_string())
            .or_else(|| conv.target_session.clone())
            .unwrap_or_else(|| "chat".to_string());

        // Publish UserChat event into the hook broadcast channel
        let event = HookEvent {
            tmux_session,
            kind: HookEventKind::UserChat {
                message: message.to_string(),
                target_session: conv.target_session.clone(),
                conversation_id: Some(conversation_id.to_string()),
            },
            session_id: String::new(),
            cwd: String::new(),
            ts: now,
            prompt: None,
            usage: None,
        };

        self.event_tx
            .send(event)
            .map_err(|_| ChatError::ChannelClosed)?;

        Ok(())
    }

    /// Non-blocking poll for chat responses. Returns all available responses.
    pub fn poll_responses(&mut self) -> Vec<ChatResponsePayload> {
        let mut responses = Vec::new();
        while let Ok(payload) = self.response_rx.try_recv() {
            // Record assistant message in conversation history
            if let Some(conv) = self.conversations.get_mut(&payload.conversation_id) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                conv.messages.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: payload.content.clone(),
                    timestamp: now,
                    conversation_id: payload.conversation_id.clone(),
                });
            }
            responses.push(payload);
        }
        responses
    }

    /// Get a reference to a conversation by ID.
    pub fn get_conversation(&self, id: &str) -> Option<&Conversation> {
        self.conversations.get(id)
    }

    /// List all conversation IDs.
    pub fn list_conversations(&self) -> Vec<&str> {
        self.conversations.keys().map(|s| s.as_str()).collect()
    }
}

/// Errors that can occur in the chat service.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("conversation not found")]
    ConversationNotFound,
    #[error("broadcast channel closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_conversation_returns_unique_ids() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (_response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        let id1 = svc.create_conversation(None);
        let id2 = svc.create_conversation(Some("session-a".into()));

        assert_ne!(id1, id2);
        assert!(id1.starts_with("conv-"));
        assert!(id2.starts_with("conv-"));
    }

    #[test]
    fn send_message_records_in_history() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (_response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        let conv_id = svc.create_conversation(None);
        svc.send_message(&conv_id, "hello", None).unwrap();

        let conv = svc.get_conversation(&conv_id).unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, ChatRole::User);
        assert_eq!(conv.messages[0].content, "hello");
    }

    #[test]
    fn send_message_publishes_hook_event() {
        let (event_tx, mut event_rx) = broadcast::channel(16);
        let (_response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        let conv_id = svc.create_conversation(Some("my-session".into()));
        svc.send_message(&conv_id, "test message", None).unwrap();

        let event = event_rx.try_recv().unwrap();
        assert!(matches!(
            event.kind,
            HookEventKind::UserChat {
                ref message,
                ref conversation_id,
                ..
            } if message == "test message" && conversation_id == &Some(conv_id.clone())
        ));
    }

    #[test]
    fn send_message_fails_on_missing_conversation() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (_response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        let result = svc.send_message("nonexistent", "hello", None);
        assert!(result.is_err());
    }

    #[test]
    fn poll_responses_records_assistant_messages() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        let conv_id = svc.create_conversation(None);

        // Simulate a response from the agent framework
        response_tx
            .send(ChatResponsePayload {
                conversation_id: conv_id.clone(),
                content: "hi there".into(),
                is_complete: true,
                session_key: None,
            })
            .unwrap();

        let responses = svc.poll_responses();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].content, "hi there");

        let conv = svc.get_conversation(&conv_id).unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, ChatRole::Assistant);
    }

    #[test]
    fn list_conversations_returns_all_ids() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (_response_tx, response_rx) = mpsc::unbounded_channel();
        let mut svc = ChatService::new(event_tx, response_rx);

        svc.create_conversation(None);
        svc.create_conversation(None);

        let ids = svc.list_conversations();
        assert_eq!(ids.len(), 2);
    }
}
