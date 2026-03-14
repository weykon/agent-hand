//! Notification manager: maps status events to CESP categories and plays sounds.

use std::time::{Duration, Instant};

use super::pack::SoundPack;

/// CESP event categories
const CAT_TASK_COMPLETE: &str = "task.complete";
const CAT_INPUT_REQUIRED: &str = "input.required";
const CAT_TASK_ERROR: &str = "task.error";
const CAT_SESSION_START: &str = "session.start";
const CAT_TASK_ACK: &str = "task.acknowledge";
const CAT_RESOURCE_LIMIT: &str = "resource.limit";
const CAT_USER_SPAM: &str = "user.spam";

/// Spam detection window
const SPAM_WINDOW_SECS: u64 = 5;
/// Number of prompts within the window to trigger spam
const SPAM_THRESHOLD: usize = 3;

/// Manages sound notifications for session status transitions.
pub struct NotificationManager {
    pack: Option<SoundPack>,
    config: crate::config::NotificationConfig,
    /// Debounce: last notification time per session
    last_notify: std::collections::HashMap<String, Instant>,
    /// Minimum interval between notifications (seconds)
    cooldown: Duration,
    /// Per-session prompt timestamps for spam detection
    prompt_times: std::collections::HashMap<String, Vec<Instant>>,
    /// Spam detection window
    spam_window: Duration,
    /// Spam detection threshold (prompts within window)
    spam_threshold: usize,
}

impl NotificationManager {
    /// Create from config. Loads the sound pack if available.
    pub fn new(config: &crate::config::NotificationConfig) -> Self {
        let pack = if config.enabled {
            SoundPack::load(&config.sound_pack)
        } else {
            None
        };

        if pack.is_none() && config.enabled {
            tracing::info!(
                "Sound pack '{}' not found — notifications will be silent. \
                 Install peon-ping packs to ~/.openpeon/packs/ for sound support.",
                config.sound_pack
            );
        }

        Self {
            pack,
            config: config.clone(),
            last_notify: std::collections::HashMap::new(),
            cooldown: Duration::from_secs(3),
            prompt_times: std::collections::HashMap::new(),
            spam_window: Duration::from_secs(SPAM_WINDOW_SECS),
            spam_threshold: SPAM_THRESHOLD,
        }
    }

    /// Notify: task completed (Running → Idle)
    pub fn on_task_complete(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_task_complete {
            return;
        }
        self.play_category(session_id, CAT_TASK_COMPLETE);
    }

    /// Notify: input required (→ Waiting)
    pub fn on_input_required(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_input_required {
            return;
        }
        self.play_category(session_id, CAT_INPUT_REQUIRED);
    }

    /// Notify: tool failure
    pub fn on_error(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_error {
            return;
        }
        self.play_category(session_id, CAT_TASK_ERROR);
    }

    /// Notify: session started working (non-Running → Running)
    pub fn on_session_start(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_session_start {
            return;
        }
        self.play_category(session_id, CAT_SESSION_START);
    }

    /// Notify: prompt received while session already running
    pub fn on_task_acknowledge(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_task_acknowledge {
            return;
        }
        self.play_category(session_id, CAT_TASK_ACK);
    }

    /// Notify: context window about to compact
    pub fn on_resource_limit(&mut self, session_id: &str) {
        if !self.config.enabled || !self.config.on_resource_limit {
            return;
        }
        self.play_category(session_id, CAT_RESOURCE_LIMIT);
    }

    /// Check for prompt spam and play spam sound if detected.
    /// Returns `true` if spam was detected (caller should skip start/ack sound).
    pub fn on_user_prompt(&mut self, session_id: &str) -> bool {
        if !self.config.enabled || !self.config.on_user_spam {
            return false;
        }

        let now = Instant::now();
        let times = self
            .prompt_times
            .entry(session_id.to_string())
            .or_default();

        // Record current prompt
        times.push(now);

        // Evict timestamps outside the spam window
        let window = self.spam_window;
        times.retain(|t| now.duration_since(*t) < window);

        // Check threshold
        if times.len() >= self.spam_threshold {
            self.play_category(session_id, CAT_USER_SPAM);
            return true;
        }

        false
    }

    pub fn play_category(&mut self, session_id: &str, category: &str) {
        // Cooldown check
        let now = Instant::now();
        if let Some(last) = self.last_notify.get(session_id) {
            if now.duration_since(*last) < self.cooldown {
                return;
            }
        }
        self.last_notify.insert(session_id.to_string(), now);

        // Pick and play sound
        if let Some(ref pack) = self.pack {
            if let Some(path) = pack.pick_sound(category) {
                super::sound::play_async(&path, self.config.volume);
            }
        }
    }

    /// Reload pack (e.g., after config change)
    pub fn reload_pack(&mut self, config: &crate::config::NotificationConfig) {
        self.config = config.clone();
        self.pack = if config.enabled {
            SoundPack::load(&config.sound_pack)
        } else {
            None
        };
    }
}
