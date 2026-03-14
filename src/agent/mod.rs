//! Agent framework — lightweight ECS for reactive event processing.
//!
//! The pattern: HookEvent → World update → System dispatch → Action execution
//!
//! - **Entity**: each tmux session (keyed by session name)
//! - **Component**: per-session state (prev/current status, project path)
//! - **System**: reactive logic that maps events to actions (e.g. SoundSystem)
//! - **Action**: side effects produced by Systems, executed asynchronously

pub mod analyzer;
pub mod analyzer_host;
pub mod consumers;
pub mod delivery;
pub mod guard;
pub mod hot_brain;
pub mod io;
pub mod memory;
pub mod projections;
pub mod runner;
pub mod scheduler;
pub mod systems;
#[cfg(feature = "wasm")]
pub mod wasm_canvas;
#[cfg(feature = "wasm")]
pub mod wasm_executor;
#[cfg(feature = "wasm")]
pub mod wasm_host;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::hooks::{HookEvent, HookEventKind};
use crate::session::Status;

/// A reactive system that processes hook events and produces actions.
///
/// Systems are **synchronous**: they receive an event + read-only World access,
/// and return a list of Actions. Slow operations (sound playback, file I/O, AI calls)
/// happen in the Action execution layer, not here.
pub trait System: Send + 'static {
    /// Human-readable name for logging/debugging.
    fn name(&self) -> &'static str;

    /// Process a hook event and return zero or more actions.
    fn on_event(&mut self, event: &HookEvent, world: &World) -> Vec<Action>;
}

/// World holds per-session entity state. Systems get read-only access.
///
/// Kept minimal — only tracks what persists across events.
pub struct World {
    /// Per-session status tracking (keyed by tmux session name).
    pub sessions: HashMap<String, SessionState>,
}

/// Lightweight per-session state (the "components" of our entity).
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Status before the current event was processed.
    pub prev_status: Status,
    /// Status after the current event was processed.
    pub current_status: Status,
    /// Working directory / project path (populated from HookEvent.cwd).
    pub project_path: Option<PathBuf>,
    /// Claude Code session_id (for sub-agent tracking).
    pub session_id: Option<String>,
}

/// Side effects produced by Systems, executed asynchronously by ActionExecutor.
#[derive(Debug)]
pub enum Action {
    /// Play a CESP sound category for a session.
    PlaySound {
        category: String,
        session_key: String,
    },
    /// Write a progress entry to the session's progress file (Anthropic harness).
    WriteProgress {
        session_key: String,
        entry: ProgressEntry,
    },
    /// Guarded context injection — proposal passed through guard pipeline.
    GuardedContextInjection {
        session_key: String,
        project_path: PathBuf,
        commit: guard::GuardedCommit,
        evidence: Vec<guard::EvidenceRecord>,
        proposal: guard::Proposal,
        feedback_packet: Option<guard::FeedbackPacket>,
    },
    /// Append an arbitrary JSON record to a runtime audit stream.
    AuditJson {
        filename: String,
        record: serde_json::Value,
    },
    /// Log a message (for debugging/tracing).
    Log { message: String },
    /// Chat response to be streamed back to the user (TUI panel or CLI REPL).
    ChatResponse {
        /// Conversation ID for routing
        conversation_id: String,
        /// Response content (full or chunk for streaming)
        content: String,
        /// Whether this is the final chunk
        is_complete: bool,
        /// Optional session context this response relates to
        session_key: Option<String>,
    },
    /// WASM canvas event from TUI (e.g. node click).
    #[cfg(feature = "wasm")]
    WasmCanvasEvent {
        event_type: String,
        node_id: Option<String>,
        canvas_summary: Option<wasm_canvas::CanvasSummary>,
    },
}

/// Progress log entries — the "external artifact" pattern from Anthropic's harness design.
///
/// These are appended to `~/.agent-hand/profiles/default/progress/{tmux_name}.md`
/// and serve as durable memory across context-window compactions.
#[derive(Debug, Clone)]
pub enum ProgressEntry {
    /// Agent finished a task (Running → Idle).
    TaskComplete { ts: f64 },
    /// Context window about to compact — checkpoint of current state.
    PreCompactSave { ts: f64 },
    /// A tool call failed.
    Error { ts: f64, tool: String, error: String },
}

impl World {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Update entity state from a hook event.
    /// Called once per event, before dispatching to Systems.
    pub fn update_from_event(&mut self, event: &HookEvent) {
        let entry = self
            .sessions
            .entry(event.tmux_session.clone())
            .or_insert_with(|| SessionState {
                prev_status: Status::Idle,
                current_status: Status::Idle,
                project_path: None,
                session_id: None,
            });

        // UserChat is sideband — don't change session status.
        if !matches!(event.kind, HookEventKind::UserChat { .. }) {
            let new_status = event_to_status(&event.kind);
            entry.prev_status = entry.current_status;
            entry.current_status = new_status;
        }

        // Update project_path from cwd if available
        if !event.cwd.is_empty() {
            entry.project_path = Some(PathBuf::from(&event.cwd));
        }

        // Update session_id if available
        if !event.session_id.is_empty() {
            entry.session_id = Some(event.session_id.clone());
        }
    }
}

/// Map a hook event kind to a session status.
/// Canonical mapping used by all Systems.
pub fn event_to_status(kind: &HookEventKind) -> Status {
    match kind {
        HookEventKind::UserPromptSubmit => Status::Running,
        HookEventKind::Stop => Status::Idle,
        HookEventKind::Notification { notification_type } => {
            match notification_type.as_str() {
                "idle_prompt" => Status::Idle,
                "elicitation_dialog" | "permission_prompt" => Status::Waiting,
                _ => Status::Idle,
            }
        }
        HookEventKind::PermissionRequest { .. } => Status::Waiting,
        HookEventKind::ToolFailure { .. } => Status::Idle,
        HookEventKind::SubagentStart => Status::Running,
        HookEventKind::PreCompact => Status::Running,
        // UserChat is sideband — does not change session status.
        // Preserve whatever status the session already has.
        HookEventKind::UserChat { .. } => Status::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: HookEventKind, cwd: &str) -> HookEvent {
        HookEvent {
            tmux_session: "test_session".to_string(),
            kind,
            session_id: "sid-123".to_string(),
            cwd: cwd.to_string(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        }
    }

    #[test]
    fn world_update_tracks_status_transitions() {
        let mut world = World::new();

        // First event: UserPromptSubmit → Idle→Running
        let event = make_event(HookEventKind::UserPromptSubmit, "/tmp/proj");
        world.update_from_event(&event);

        let state = world.sessions.get("test_session").unwrap();
        assert_eq!(state.prev_status, Status::Idle);
        assert_eq!(state.current_status, Status::Running);
        assert_eq!(state.project_path, Some(PathBuf::from("/tmp/proj")));
        assert_eq!(state.session_id, Some("sid-123".to_string()));

        // Second event: Stop → Running→Idle
        let event = make_event(HookEventKind::Stop, "/tmp/proj");
        world.update_from_event(&event);

        let state = world.sessions.get("test_session").unwrap();
        assert_eq!(state.prev_status, Status::Running);
        assert_eq!(state.current_status, Status::Idle);
    }

    #[test]
    fn world_update_captures_cwd_and_session_id() {
        let mut world = World::new();
        let event = make_event(HookEventKind::UserPromptSubmit, "/home/user/project");
        world.update_from_event(&event);

        let state = world.sessions.get("test_session").unwrap();
        assert_eq!(state.project_path, Some(PathBuf::from("/home/user/project")));
        assert_eq!(state.session_id, Some("sid-123".to_string()));
    }

    #[test]
    fn world_update_ignores_empty_cwd() {
        let mut world = World::new();
        let event = make_event(HookEventKind::UserPromptSubmit, "");
        world.update_from_event(&event);

        let state = world.sessions.get("test_session").unwrap();
        assert_eq!(state.project_path, None);
    }

    #[test]
    fn progress_system_produces_actions_on_stop() {
        use systems::progress::ProgressSystem;

        let mut sys = ProgressSystem;
        let world = World::new();
        let event = make_event(HookEventKind::Stop, "");

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Action::WriteProgress {
            entry: ProgressEntry::TaskComplete { .. }, ..
        }));
    }

    #[test]
    fn progress_system_produces_actions_on_pre_compact() {
        use systems::progress::ProgressSystem;

        let mut sys = ProgressSystem;
        let world = World::new();
        let event = make_event(HookEventKind::PreCompact, "");

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Action::WriteProgress {
            entry: ProgressEntry::PreCompactSave { .. }, ..
        }));
    }

    #[test]
    fn progress_system_produces_actions_on_tool_failure() {
        use systems::progress::ProgressSystem;

        let mut sys = ProgressSystem;
        let world = World::new();
        let event = make_event(
            HookEventKind::ToolFailure {
                tool_name: "Bash".to_string(),
                error: "not found".to_string(),
            },
            "",
        );

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Action::WriteProgress {
            entry: ProgressEntry::Error { .. }, ..
        }));
    }

    #[test]
    fn progress_system_ignores_prompt_events() {
        use systems::progress::ProgressSystem;

        let mut sys = ProgressSystem;
        let world = World::new();
        let event = make_event(HookEventKind::UserPromptSubmit, "");

        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty());
    }

    #[test]
    fn context_guard_system_produces_injection_on_prompt_with_path() {
        use crate::config::ContextBridgeConfig;
        use systems::context::ContextGuardSystem;

        let mut sys = ContextGuardSystem::new(ContextBridgeConfig::default(), std::path::PathBuf::from("/tmp/agent-hand-test-runtime"));
        let mut world = World::new();

        // First populate the world with a project path
        let setup_event = make_event(HookEventKind::Stop, "/tmp/proj");
        world.update_from_event(&setup_event);

        // Now send a UserPromptSubmit
        let event = make_event(HookEventKind::UserPromptSubmit, "/tmp/proj");
        world.update_from_event(&event);

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Action::GuardedContextInjection {
            project_path, ..
        } if project_path == &PathBuf::from("/tmp/proj")));
    }

    #[test]
    fn context_guard_system_does_not_trigger_on_stop_by_default() {
        use crate::config::ContextBridgeConfig;
        use systems::context::ContextGuardSystem;

        let mut sys = ContextGuardSystem::new(ContextBridgeConfig::default(), std::path::PathBuf::from("/tmp/agent-hand-test-runtime"));
        let mut world = World::new();

        // Populate world
        let setup_event = make_event(HookEventKind::UserPromptSubmit, "/tmp/proj");
        world.update_from_event(&setup_event);

        // Stop event should NOT trigger (default config only triggers on user_prompt_submit)
        let event = make_event(HookEventKind::Stop, "/tmp/proj");
        world.update_from_event(&event);

        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty(), "Stop should not trigger with default config");
    }

    #[test]
    fn context_guard_system_skips_irrelevant_events() {
        use crate::config::ContextBridgeConfig;
        use systems::context::ContextGuardSystem;

        let mut sys = ContextGuardSystem::new(ContextBridgeConfig::default(), std::path::PathBuf::from("/tmp/agent-hand-test-runtime"));
        let world = World::new();
        let event = make_event(HookEventKind::SubagentStart, "/tmp/proj");

        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty());
    }

    #[test]
    fn context_guard_system_skips_when_no_project_path() {
        use crate::config::ContextBridgeConfig;
        use systems::context::ContextGuardSystem;

        let mut sys = ContextGuardSystem::new(ContextBridgeConfig::default(), std::path::PathBuf::from("/tmp/agent-hand-test-runtime"));
        let mut world = World::new();

        // Insert a session state WITHOUT project_path
        world.sessions.insert(
            "test_session".to_string(),
            SessionState {
                prev_status: Status::Idle,
                current_status: Status::Running,
                project_path: None,
                session_id: None,
            },
        );

        let event = make_event(HookEventKind::UserPromptSubmit, "");
        let actions = sys.on_event(&event, &world);
        assert!(actions.is_empty());
    }

    #[test]
    fn event_to_status_mapping() {
        assert_eq!(event_to_status(&HookEventKind::UserPromptSubmit), Status::Running);
        assert_eq!(event_to_status(&HookEventKind::Stop), Status::Idle);
        assert_eq!(event_to_status(&HookEventKind::SubagentStart), Status::Running);
        assert_eq!(event_to_status(&HookEventKind::PreCompact), Status::Running);
        assert_eq!(
            event_to_status(&HookEventKind::PermissionRequest {
                tool_name: "Bash".into()
            }),
            Status::Waiting
        );
        assert_eq!(
            event_to_status(&HookEventKind::ToolFailure {
                tool_name: "Read".into(),
                error: "err".into()
            }),
            Status::Idle
        );
        // UserChat maps to Idle in event_to_status, but World skips the update
        assert_eq!(
            event_to_status(&HookEventKind::UserChat {
                message: "hi".into(),
                target_session: None,
                conversation_id: None,
            }),
            Status::Idle
        );
    }

    #[test]
    fn user_chat_does_not_change_session_status() {
        let mut world = World::new();

        // Set session to Running
        let event = make_event(HookEventKind::UserPromptSubmit, "/tmp/proj");
        world.update_from_event(&event);
        assert_eq!(
            world.sessions.get("test_session").unwrap().current_status,
            Status::Running
        );

        // UserChat should NOT change status
        let chat_event = HookEvent {
            tmux_session: "test_session".to_string(),
            kind: HookEventKind::UserChat {
                message: "hello".to_string(),
                target_session: None,
                conversation_id: Some("conv-1".to_string()),
            },
            session_id: "sid-123".to_string(),
            cwd: "/tmp/proj".to_string(),
            ts: 1700000001.0,
            prompt: None,
            usage: None,
        };
        world.update_from_event(&chat_event);

        let state = world.sessions.get("test_session").unwrap();
        assert_eq!(state.current_status, Status::Running, "UserChat must not change status");
        assert_eq!(state.prev_status, Status::Idle, "prev_status should not change on UserChat");
    }

    #[test]
    fn chat_system_produces_response() {
        use systems::chat::ChatSystem;

        let mut sys = ChatSystem::new();
        let world = World::new();
        let event = HookEvent {
            tmux_session: "test_session".to_string(),
            kind: HookEventKind::UserChat {
                message: "ping".to_string(),
                target_session: None,
                conversation_id: Some("conv-42".to_string()),
            },
            session_id: "sid-123".to_string(),
            cwd: String::new(),
            ts: 1700000000.0,
            prompt: None,
            usage: None,
        };

        let actions = sys.on_event(&event, &world);
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::AuditJson { .. }));
        assert!(matches!(&actions[1], Action::ChatResponse { content, .. } if content == "[echo] ping"));
    }
}
