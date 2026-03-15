//! ToolActivitySystem — lightweight audit of tool usage.
//!
//! Only reacts to PostToolUse events. Emits an AuditJson action
//! every AUDIT_INTERVAL tool calls per session.

use std::collections::HashMap;

use crate::hooks::{HookEvent, HookEventKind};

use super::super::{Action, System, World};

/// How often to emit a tool activity audit record (every N PostToolUse events per session).
const AUDIT_INTERVAL: u64 = 25;

/// Tracks per-session PostToolUse counts to know when to emit audit records.
pub struct ToolActivitySystem {
    post_counts: HashMap<String, u64>,
}

impl ToolActivitySystem {
    pub fn new() -> Self {
        Self {
            post_counts: HashMap::new(),
        }
    }
}

impl System for ToolActivitySystem {
    fn name(&self) -> &'static str {
        "tool_activity"
    }

    fn on_event(&mut self, event: &HookEvent, world: &World) -> Vec<Action> {
        // Only react to PostToolUse
        let (tool_name, tool_use_id) = match &event.kind {
            HookEventKind::PostToolUse {
                tool_name,
                tool_use_id,
                ..
            } => (tool_name.as_str(), tool_use_id.as_str()),
            _ => return vec![],
        };

        let count = self
            .post_counts
            .entry(event.tmux_session.clone())
            .or_insert(0);
        *count += 1;

        if *count % AUDIT_INTERVAL != 0 {
            return vec![];
        }

        // Build audit record from world state
        let state = match world.sessions.get(&event.tmux_session) {
            Some(s) => s,
            None => return vec![],
        };

        let top_tools: Vec<_> = {
            let mut pairs: Vec<_> = state.tool_history.counts_by_tool.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1));
            pairs.truncate(10);
            pairs
        };

        let record = serde_json::json!({
            "session": event.tmux_session,
            "total_count": state.tool_history.total_count,
            "latest_tool": tool_name,
            "latest_tool_use_id": tool_use_id,
            "tool_frequencies": top_tools.iter().map(|(k, v)| {
                serde_json::json!({"tool": k, "count": v})
            }).collect::<Vec<_>>(),
            "ts": event.ts,
        });

        vec![Action::AuditJson {
            filename: "tool_activity.jsonl".to_string(),
            record,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::HookEvent;

    fn make_post_tool_event(session: &str, tool: &str, id: &str) -> HookEvent {
        HookEvent {
            tmux_session: session.to_string(),
            kind: HookEventKind::PostToolUse {
                tool_name: tool.to_string(),
                tool_input: serde_json::Value::Null,
                tool_response: String::new(),
                tool_use_id: id.to_string(),
            },
            session_id: "sid-1".to_string(),
            cwd: "/tmp".to_string(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        }
    }

    #[test]
    fn ignores_non_tool_events() {
        let mut sys = ToolActivitySystem::new();
        let world = World::new();
        let event = HookEvent {
            tmux_session: "s1".to_string(),
            kind: HookEventKind::UserPromptSubmit,
            session_id: "sid-1".to_string(),
            cwd: String::new(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        };
        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty());
    }

    #[test]
    fn emits_audit_at_interval() {
        let mut sys = ToolActivitySystem::new();
        let mut world = World::new();

        // Populate the world with tool history via PreToolUse events first
        for i in 0..AUDIT_INTERVAL {
            let pre = HookEvent {
                tmux_session: "s1".to_string(),
                kind: HookEventKind::PreToolUse {
                    tool_name: "Bash".to_string(),
                    tool_input: serde_json::Value::Null,
                    tool_use_id: format!("tu-{}", i),
                },
                session_id: "sid-1".to_string(),
                cwd: "/tmp".to_string(),
                ts: 1700000000.0 + i as f64,
                prompt: None,
                usage: None,
            };
            world.update_from_event(&pre);

            let post = make_post_tool_event("s1", "Bash", &format!("tu-{}", i));
            world.update_from_event(&post);

            let actions = sys.on_event(&post, &world);
            if i < AUDIT_INTERVAL - 1 {
                assert!(actions.is_empty(), "should not emit at count {}", i + 1);
            } else {
                assert_eq!(actions.len(), 1, "should emit at count {}", AUDIT_INTERVAL);
                assert!(matches!(&actions[0], Action::AuditJson { filename, .. } if filename == "tool_activity.jsonl"));
            }
        }
    }
}
