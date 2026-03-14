//! Background sound notification task.
//!
//! Runs independently of the main TUI event loop so that sound notifications
//! continue playing even while the user is attached to a tmux session
//! (which blocks the main `tokio::select!` loop).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::broadcast;

use crate::config::NotificationConfig;
use crate::hooks::{HookEvent, HookEventKind};
use crate::session::Status;

/// Shared state for tracking which tmux session the user is currently attached to.
/// `None` means the user is on the dashboard (not inside any session).
pub type AttachedSession = Arc<RwLock<Option<String>>>;

/// Shared notification config that can be hot-reloaded from the settings dialog.
pub type SharedNotificationConfig = Arc<RwLock<NotificationConfig>>;

/// Spawn the background sound notification task.
///
/// This function runs forever, consuming hook events from the broadcast channel
/// and playing sounds based on status transitions. It is completely independent
/// of the main event loop.
pub async fn run_sound_notifications(
    mut rx: broadcast::Receiver<HookEvent>,
    config: SharedNotificationConfig,
    attached_session: AttachedSession,
) {
    let initial_config = config.read().unwrap_or_else(|e| e.into_inner()).clone();
    let mut manager = crate::notification::NotificationManager::new(&initial_config);

    // Per-session status tracking (keyed by tmux session name)
    let mut previous_statuses: HashMap<String, Status> = HashMap::new();

    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("Sound task: skipped {n} events (lagged)");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::debug!("Sound task: broadcast channel closed, exiting");
                break;
            }
        };

        // Hot-reload config if it changed
        if let Ok(cfg) = config.read() {
            manager.reload_pack(&cfg);
        }

        // Check quiet_when_focused: skip sound if the event is from the
        // currently attached session and the config says to be quiet
        let should_quiet = {
            let quiet_enabled = config
                .read()
                .map(|c| c.quiet_when_focused)
                .unwrap_or(false);
            if quiet_enabled {
                attached_session
                    .read()
                    .map(|a| a.as_deref() == Some(&event.tmux_session))
                    .unwrap_or(false)
            } else {
                false
            }
        };

        if should_quiet {
            // Still update status tracking so transitions are correct when
            // the user detaches, but don't play any sound.
            let new_status = event_to_status(&event.kind);
            previous_statuses.insert(event.tmux_session.clone(), new_status);
            continue;
        }

        // Map event → status
        let new_status = event_to_status(&event.kind);
        let prev_status = previous_statuses
            .get(&event.tmux_session)
            .copied()
            .unwrap_or(Status::Idle);

        // Use tmux session name as the notification key (for cooldown/debounce)
        let key = &event.tmux_session;

        // Detect Running → Done (task complete)
        if prev_status == Status::Running
            && (new_status == Status::Idle || new_status == Status::Waiting)
        {
            manager.on_task_complete(key);
        }

        // Detect → Waiting (input required)
        if new_status == Status::Waiting && prev_status != Status::Waiting {
            manager.on_input_required(key);
        }

        // Tool failure → error sound
        if matches!(event.kind, HookEventKind::ToolFailure { .. }) {
            manager.on_error(key);
        }

        // User prompt submitted
        if matches!(event.kind, HookEventKind::UserPromptSubmit) {
            // Check spam first — if spam detected, skip start/ack
            if !manager.on_user_prompt(key) {
                if prev_status == Status::Running {
                    manager.on_task_acknowledge(key);
                } else {
                    manager.on_session_start(key);
                }
            }
        }

        // Context compaction (resource limit)
        if matches!(event.kind, HookEventKind::PreCompact) {
            manager.on_resource_limit(key);
        }

        // Update tracked status
        previous_statuses.insert(event.tmux_session.clone(), new_status);
    }
}

/// Map a hook event kind to a session status (same logic as refresh_statuses).
fn event_to_status(kind: &HookEventKind) -> Status {
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
        HookEventKind::UserChat { .. } => Status::Idle,
    }
}
