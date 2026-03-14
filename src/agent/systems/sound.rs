//! SoundSystem — migrated from sound_task.rs.
//!
//! Maps hook events to CESP sound categories based on status transitions.
//! Produces Action::PlaySound for the ActionExecutor to play.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::config::NotificationConfig;
use crate::hooks::{HookEvent, HookEventKind};
use crate::session::Status;

use super::super::{Action, System, World};

/// Shared notification config that can be hot-reloaded from the settings dialog.
pub type SharedNotificationConfig = Arc<RwLock<NotificationConfig>>;

/// Shared state for tracking which tmux session the user is currently attached to.
pub type AttachedSession = Arc<RwLock<Option<String>>>;

/// Spam detection window (seconds).
const SPAM_WINDOW_SECS: u64 = 5;
/// Number of prompts within the window to trigger spam.
const SPAM_THRESHOLD: usize = 3;

/// Reactive system that maps status transitions to CESP sound categories.
pub struct SoundSystem {
    config: SharedNotificationConfig,
    attached_session: AttachedSession,
    /// Per-session prompt timestamps for spam detection.
    prompt_times: HashMap<String, Vec<Instant>>,
    spam_window: Duration,
    spam_threshold: usize,
}

impl SoundSystem {
    pub fn new(config: SharedNotificationConfig, attached_session: AttachedSession) -> Self {
        Self {
            config,
            attached_session,
            prompt_times: HashMap::new(),
            spam_window: Duration::from_secs(SPAM_WINDOW_SECS),
            spam_threshold: SPAM_THRESHOLD,
        }
    }

    /// Check quiet_when_focused: skip sound if the event is from the
    /// currently attached session and the config says to be quiet.
    fn should_quiet(&self, tmux_session: &str) -> bool {
        let quiet_enabled = self
            .config
            .read()
            .map(|c| c.quiet_when_focused)
            .unwrap_or(false);
        if !quiet_enabled {
            return false;
        }
        self.attached_session
            .read()
            .map(|a| a.as_deref() == Some(tmux_session))
            .unwrap_or(false)
    }

    /// Check if the given event type is enabled in config.
    fn is_enabled(&self, check: impl FnOnce(&NotificationConfig) -> bool) -> bool {
        self.config
            .read()
            .map(|c| c.enabled && check(&c))
            .unwrap_or(false)
    }

    /// Detect prompt spam. Returns true if spam detected.
    fn check_spam(&mut self, session_key: &str) -> bool {
        if !self.is_enabled(|c| c.on_user_spam) {
            return false;
        }

        let now = Instant::now();
        let times = self
            .prompt_times
            .entry(session_key.to_string())
            .or_default();

        times.push(now);

        let window = self.spam_window;
        times.retain(|t| now.duration_since(*t) < window);

        times.len() >= self.spam_threshold
    }
}

impl System for SoundSystem {
    fn name(&self) -> &'static str {
        "sound"
    }

    fn on_event(&mut self, event: &HookEvent, world: &World) -> Vec<Action> {
        // Quiet check: suppress if user is focused on this session
        if self.should_quiet(&event.tmux_session) {
            return vec![];
        }

        let key = &event.tmux_session;
        let state = world.sessions.get(key);
        let prev = state.map(|s| s.prev_status).unwrap_or(Status::Idle);
        let curr = state.map(|s| s.current_status).unwrap_or(Status::Idle);

        let mut actions = Vec::new();

        // Running → (Idle|Waiting) = task complete
        if prev == Status::Running && (curr == Status::Idle || curr == Status::Waiting) {
            if self.is_enabled(|c| c.on_task_complete) {
                actions.push(Action::PlaySound {
                    category: "task.complete".into(),
                    session_key: key.clone(),
                });
            }
        }

        // → Waiting (input required)
        if curr == Status::Waiting && prev != Status::Waiting {
            if self.is_enabled(|c| c.on_input_required) {
                actions.push(Action::PlaySound {
                    category: "input.required".into(),
                    session_key: key.clone(),
                });
            }
        }

        // Tool failure
        if matches!(event.kind, HookEventKind::ToolFailure { .. }) {
            if self.is_enabled(|c| c.on_error) {
                actions.push(Action::PlaySound {
                    category: "task.error".into(),
                    session_key: key.clone(),
                });
            }
        }

        // User prompt submitted
        if matches!(event.kind, HookEventKind::UserPromptSubmit) {
            if self.check_spam(key) {
                // Spam detected — play spam sound, skip start/ack
                actions.push(Action::PlaySound {
                    category: "user.spam".into(),
                    session_key: key.clone(),
                });
            } else if prev == Status::Running {
                if self.is_enabled(|c| c.on_task_acknowledge) {
                    actions.push(Action::PlaySound {
                        category: "task.acknowledge".into(),
                        session_key: key.clone(),
                    });
                }
            } else {
                if self.is_enabled(|c| c.on_session_start) {
                    actions.push(Action::PlaySound {
                        category: "session.start".into(),
                        session_key: key.clone(),
                    });
                }
            }
        }

        // Context compaction (resource limit)
        if matches!(event.kind, HookEventKind::PreCompact) {
            if self.is_enabled(|c| c.on_resource_limit) {
                actions.push(Action::PlaySound {
                    category: "resource.limit".into(),
                    session_key: key.clone(),
                });
            }
        }

        actions
    }
}
