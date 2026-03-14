//! ChatSystem — user chat message handler.
//!
//! Processes UserChat events and produces ChatResponse actions.
//! Initial implementation echoes back; real AI integration comes via ChatService.

use serde_json::json;

use crate::hooks::{HookEvent, HookEventKind};

use super::super::{Action, System, World};

/// Handles user chat messages, producing responses and audit trails.
pub struct ChatSystem;

impl ChatSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChatSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for ChatSystem {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn on_event(&mut self, event: &HookEvent, _world: &World) -> Vec<Action> {
        let HookEventKind::UserChat {
            ref message,
            ref conversation_id,
            ..
        } = event.kind
        else {
            return vec![];
        };

        let conv_id = conversation_id
            .clone()
            .unwrap_or_else(|| format!("chat-{}", event.ts as u64));

        let mut actions = Vec::new();

        // Audit trail
        actions.push(Action::AuditJson {
            filename: "chat_history.jsonl".into(),
            record: json!({
                "type": "user_chat",
                "tmux_session": event.tmux_session,
                "conversation_id": conv_id,
                "message": message,
                "ts": event.ts,
            }),
        });

        // Echo response (placeholder until ChatService provides real AI)
        actions.push(Action::ChatResponse {
            conversation_id: conv_id,
            content: format!("[echo] {}", message),
            is_complete: true,
            session_key: Some(event.tmux_session.clone()),
        });

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{HookEvent, HookEventKind};

    fn chat_event(message: &str, conv_id: Option<&str>) -> HookEvent {
        HookEvent {
            tmux_session: "test_session".to_string(),
            kind: HookEventKind::UserChat {
                message: message.to_string(),
                target_session: None,
                conversation_id: conv_id.map(|s| s.to_string()),
            },
            session_id: "sid-123".to_string(),
            cwd: "/tmp/proj".to_string(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        }
    }

    #[test]
    fn produces_echo_response_and_audit() {
        let mut sys = ChatSystem::new();
        let world = World::new();
        let event = chat_event("hello world", Some("conv-1"));

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 2);

        // First action: audit
        assert!(matches!(&actions[0], Action::AuditJson { filename, .. } if filename == "chat_history.jsonl"));

        // Second action: echo response
        match &actions[1] {
            Action::ChatResponse {
                conversation_id,
                content,
                is_complete,
                session_key,
            } => {
                assert_eq!(conversation_id, "conv-1");
                assert_eq!(content, "[echo] hello world");
                assert!(*is_complete);
                assert_eq!(session_key.as_deref(), Some("test_session"));
            }
            other => panic!("expected ChatResponse, got {:?}", other),
        }
    }

    #[test]
    fn generates_conversation_id_when_missing() {
        let mut sys = ChatSystem::new();
        let world = World::new();
        let event = chat_event("test", None);

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 2);

        match &actions[1] {
            Action::ChatResponse {
                conversation_id, ..
            } => {
                assert!(conversation_id.starts_with("chat-"));
            }
            other => panic!("expected ChatResponse, got {:?}", other),
        }
    }

    #[test]
    fn ignores_non_chat_events() {
        let mut sys = ChatSystem::new();
        let world = World::new();
        let event = HookEvent {
            tmux_session: "test_session".to_string(),
            kind: HookEventKind::Stop,
            session_id: "sid-123".to_string(),
            cwd: String::new(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        };

        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty());
    }
}
