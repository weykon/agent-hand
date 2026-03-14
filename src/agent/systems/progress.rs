//! ProgressSystem — Anthropic harness pattern.
//!
//! Writes progress entries to durable files so that context survives
//! compaction and provides external memory for long-running agents.

use crate::hooks::{HookEvent, HookEventKind};

use super::super::{Action, ProgressEntry, System, World};

/// Tracks session progress via durable progress files.
///
/// On Stop → TaskComplete, on PreCompact → PreCompactSave,
/// on ToolFailure → Error. These entries are appended to
/// `~/.agent-hand/profiles/default/progress/{tmux_name}.md`.
pub struct ProgressSystem;

impl System for ProgressSystem {
    fn name(&self) -> &'static str {
        "progress"
    }

    fn on_event(&mut self, event: &HookEvent, _world: &World) -> Vec<Action> {
        let key = event.tmux_session.clone();

        match &event.kind {
            HookEventKind::Stop => {
                vec![Action::WriteProgress {
                    session_key: key,
                    entry: ProgressEntry::TaskComplete { ts: event.ts },
                }]
            }
            HookEventKind::PreCompact => {
                vec![Action::WriteProgress {
                    session_key: key,
                    entry: ProgressEntry::PreCompactSave { ts: event.ts },
                }]
            }
            HookEventKind::ToolFailure { tool_name, error } => {
                vec![Action::WriteProgress {
                    session_key: key,
                    entry: ProgressEntry::Error {
                        ts: event.ts,
                        tool: tool_name.clone(),
                        error: error.clone(),
                    },
                }]
            }
            _ => vec![],
        }
    }
}
