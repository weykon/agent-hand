use std::collections::HashMap;

use serde_json::json;

use crate::agent::{Action, System, World};
use crate::hooks::HookEvent;

/// Minimal derived hook over primary hook usage fields.
///
/// Purpose:
/// - detect unusually large token consumption spikes from structured hook usage
/// - emit a bounded sound/audit side effect without mutating runtime state
///
/// This is intentionally narrow: it does not drive automation, only observability.
pub struct TokenBurstSystem {
    last_total_by_session: HashMap<String, (u64, f64)>,
}

const MIN_BURST_DELTA_TOKENS: u64 = 4000;
const MIN_BURST_TOKENS_PER_SEC: f64 = 120.0;
const MIN_FIRST_EVENT_TOTAL_TOKENS: u64 = 8000;

impl TokenBurstSystem {
    pub fn new() -> Self {
        Self {
            last_total_by_session: HashMap::new(),
        }
    }
}

impl Default for TokenBurstSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for TokenBurstSystem {
    fn name(&self) -> &'static str {
        "token_burst"
    }

    fn on_event(&mut self, event: &HookEvent, _world: &World) -> Vec<Action> {
        let Some(usage) = &event.usage else {
            return vec![];
        };
        let Some(total_tokens) = usage.total_tokens else {
            return vec![];
        };

        let session_key = if !event.session_id.is_empty() {
            event.session_id.clone()
        } else {
            event.tmux_session.clone()
        };

        let mut delta_tokens = total_tokens;
        let mut tokens_per_sec = 0.0;

        if let Some((prev_total, prev_ts)) = self.last_total_by_session.get(&session_key).copied() {
            delta_tokens = total_tokens.saturating_sub(prev_total);
            let dt = (event.ts - prev_ts).max(0.001);
            tokens_per_sec = delta_tokens as f64 / dt;
        }

        self.last_total_by_session
            .insert(session_key.clone(), (total_tokens, event.ts));

        let is_burst = if delta_tokens == total_tokens {
            total_tokens >= MIN_FIRST_EVENT_TOTAL_TOKENS
        } else {
            delta_tokens >= MIN_BURST_DELTA_TOKENS || tokens_per_sec >= MIN_BURST_TOKENS_PER_SEC
        };

        if !is_burst {
            return vec![];
        }

        let record = json!({
            "type": "token_burst",
            "tmux_session": event.tmux_session,
            "session_id": event.session_id,
            "ts": event.ts,
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "delta_tokens": delta_tokens,
            "tokens_per_sec": tokens_per_sec,
        });

        vec![
            Action::PlaySound {
                category: "resource.limit".into(),
                session_key: event.tmux_session.clone(),
            },
            Action::AuditJson {
                filename: "derived_hooks.jsonl".into(),
                record,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{HookEvent, HookEventKind, HookUsage};

    fn event(total: u64, ts: f64) -> HookEvent {
        HookEvent {
            tmux_session: "tmux-a".to_string(),
            kind: HookEventKind::Stop,
            session_id: "sid-a".to_string(),
            cwd: "/tmp/proj".to_string(),
            ts,
            prompt: None,
            usage: Some(HookUsage {
                total_tokens: Some(total),
                input_tokens: Some(total / 2),
                output_tokens: Some(total / 2),
                cache_creation_tokens: None,
                cache_read_tokens: None,
            }),
        }
    }

    #[test]
    fn emits_burst_on_large_initial_total() {
        let mut system = TokenBurstSystem::new();
        let actions = system.on_event(&event(9000, 10.0), &World::new());
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::PlaySound { category, .. } if category == "resource.limit"));
        assert!(matches!(&actions[1], Action::AuditJson { filename, .. } if filename == "derived_hooks.jsonl"));
    }

    #[test]
    fn emits_burst_on_large_delta() {
        let mut system = TokenBurstSystem::new();
        let _ = system.on_event(&event(1000, 10.0), &World::new());
        let actions = system.on_event(&event(6500, 20.0), &World::new());
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn ignores_small_usage_changes() {
        let mut system = TokenBurstSystem::new();
        let _ = system.on_event(&event(1000, 10.0), &World::new());
        let actions = system.on_event(&event(1500, 20.0), &World::new());
        assert!(actions.is_empty());
    }
}
