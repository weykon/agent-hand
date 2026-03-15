//! ContextGuardSystem — guarded context injection pipeline.
//!
//! Replaces the old ContextSystem. Every context injection now goes through:
//! Proposal → Evidence → Guard → Commit → Action.
//!
//! The guard is deterministic: 8 checks, all must pass for Approve.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::ContextBridgeConfig;
use crate::hooks::{HookEvent, HookEventKind};

use super::super::guard::{
    self, EvidenceKind, EvidenceRecord, FeedbackPacket, GuardDecision, GuardedCommit,
    InjectionScope, Proposal, ProposalKind, ResponseLevel, RiskLevel,
    SidecarFeedback,
};
use super::super::{Action, System, World};

/// Read agent-written sidecar feedback for a session.
///
/// Returns `SidecarFeedback::default()` on any error (missing file, bad JSON).
fn read_sidecar(runtime_dir: &Path, session_key: &str) -> SidecarFeedback {
    let path = runtime_dir.join("sidecar").join(format!("{}.json", session_key));
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Stateful context injection system with guard pipeline.
pub struct ContextGuardSystem {
    config: ContextBridgeConfig,
    /// Per-session last-injection timestamp for cooldown enforcement.
    last_injection_ts: HashMap<String, f64>,
    /// Sessions that have already been injected in the current event dispatch.
    injected_this_event: HashSet<String>,
    /// Runtime directory for sidecar and audit files.
    runtime_dir: PathBuf,
}

impl ContextGuardSystem {
    pub fn new(config: ContextBridgeConfig, runtime_dir: PathBuf) -> Self {
        Self {
            config,
            last_injection_ts: HashMap::new(),
            injected_this_event: HashSet::new(),
            runtime_dir,
        }
    }

    /// Map a HookEventKind to its config string for trigger matching.
    fn event_kind_to_trigger_string(kind: &HookEventKind) -> Option<&'static str> {
        match kind {
            HookEventKind::UserPromptSubmit => Some("user_prompt_submit"),
            HookEventKind::Stop => Some("stop"),
            HookEventKind::PreCompact => Some("pre_compact"),
            HookEventKind::SubagentStart => Some("subagent_start"),
            HookEventKind::Notification { .. } => Some("notification"),
            HookEventKind::PermissionRequest { .. } => Some("permission_request"),
            HookEventKind::ToolFailure { .. } => Some("tool_failure"),
            HookEventKind::UserChat { .. } => None,
            HookEventKind::PreToolUse { .. } => Some("pre_tool_use"),
            HookEventKind::PostToolUse { .. } => Some("post_tool_use"),
        }
    }
}

impl System for ContextGuardSystem {
    fn name(&self) -> &'static str {
        "context_guard"
    }

    fn on_event(&mut self, event: &HookEvent, world: &World) -> Vec<Action> {
        // Reset per-event dedup
        self.injected_this_event.clear();

        // Check if this event kind is in our trigger list
        let trigger_str = match Self::event_kind_to_trigger_string(&event.kind) {
            Some(s) => s,
            None => return vec![],
        };
        if !self.config.trigger_events.iter().any(|t| t == trigger_str) {
            return vec![];
        }

        // Get session state
        let state = match world.sessions.get(&event.tmux_session) {
            Some(s) => s,
            None => return vec![],
        };

        let project_path = match &state.project_path {
            Some(p) => p.clone(),
            None => return vec![],
        };

        // ── Build pipeline objects ──────────────────────────────────

        let trace_id = guard::short_id();
        let ts_ms = (event.ts * 1000.0) as u64;

        // Proposal
        let proposal = Proposal {
            id: guard::short_id(),
            trace_id: trace_id.clone(),
            session_key: event.tmux_session.clone(),
            source_session_id: state
                .session_id
                .clone()
                .unwrap_or_default(),
            kind: ProposalKind::InjectContext,
            project_path: project_path.clone(),
            scope: InjectionScope::from_config_str(&self.config.scope),
            risk: RiskLevel::Low,
            created_at_ms: ts_ms,
        };

        // Evidence records
        let mut evidence = vec![
            EvidenceRecord {
                id: guard::short_id(),
                trace_id: trace_id.clone(),
                session_key: event.tmux_session.clone(),
                kind: EvidenceKind::SessionState,
                captured_at_ms: ts_ms,
                data: serde_json::json!({
                    "prev_status": format!("{:?}", state.prev_status),
                    "current_status": format!("{:?}", state.current_status),
                    "project_path": project_path.display().to_string(),
                    "session_id": state.session_id,
                }),
            },
            EvidenceRecord {
                id: guard::short_id(),
                trace_id: trace_id.clone(),
                session_key: event.tmux_session.clone(),
                kind: EvidenceKind::EventMetadata,
                captured_at_ms: ts_ms,
                data: serde_json::json!({
                    "event_kind": trigger_str,
                    "timestamp": event.ts,
                    "tmux_session": event.tmux_session,
                }),
            },
        ];

        // Tool activity evidence (if any tool calls have been recorded)
        if state.tool_history.total_count > 0 {
            let recent_tools: Vec<&str> = state
                .tool_history
                .recent
                .iter()
                .rev()
                .take(10)
                .map(|r| r.tool_name.as_str())
                .collect();
            let top_tools: Vec<(&String, &u64)> = {
                let mut pairs: Vec<_> = state.tool_history.counts_by_tool.iter().collect();
                pairs.sort_by(|a, b| b.1.cmp(a.1));
                pairs.truncate(10);
                pairs
            };
            evidence.push(EvidenceRecord {
                id: guard::short_id(),
                trace_id: trace_id.clone(),
                session_key: event.tmux_session.clone(),
                kind: EvidenceKind::ToolActivity,
                captured_at_ms: ts_ms,
                data: serde_json::json!({
                    "total_tool_calls": state.tool_history.total_count,
                    "recent_tools": recent_tools,
                    "top_tools": top_tools.iter().map(|(k, v)| {
                        serde_json::json!({"tool": k, "count": v})
                    }).collect::<Vec<_>>(),
                }),
            });
        }

        // ── Run guard ──────────────────────────────────────────────

        let already_injected = self.injected_this_event.contains(&event.tmux_session);
        let last_ts = self.last_injection_ts.get(&event.tmux_session).copied();

        let (decision, attestation) = guard::run_guard(
            &proposal,
            &evidence,
            &self.config,
            last_ts,
            event.ts,
            already_injected,
        );

        // Build commit
        let commit = GuardedCommit {
            commit_id: guard::short_id(),
            trace_id: trace_id.clone(),
            session_key: event.tmux_session.clone(),
            proposal_id: proposal.id.clone(),
            decision: decision.clone(),
            attestation,
            created_at_ms: ts_ms,
        };

        // ── Produce action based on decision ───────────────────────

        match decision {
            GuardDecision::Approve => {
                // Update tracking state
                self.last_injection_ts
                    .insert(event.tmux_session.clone(), event.ts);
                self.injected_this_event
                    .insert(event.tmux_session.clone());

                // Read sidecar feedback written by the agent (if any).
                // Missing file → SidecarFeedback::default() → same as pre-sidecar behavior.
                let sidecar = read_sidecar(&self.runtime_dir, &event.tmux_session);
                let urgency_level = guard::parse_urgency(&sidecar.urgency);
                let recommended_response_level = match &urgency_level {
                    RiskLevel::Critical | RiskLevel::High => ResponseLevel::L3CrossSessionInject,
                    RiskLevel::Medium => ResponseLevel::L2SelfInject,
                    RiskLevel::Low => ResponseLevel::L1RecordOnly,
                };

                let feedback_packet = FeedbackPacket {
                    packet_id: guard::short_id(),
                    trace_id,
                    source_session_id: state
                        .session_id
                        .clone()
                        .unwrap_or_default(),
                    created_at_ms: ts_ms,
                    goal: sidecar.goal,
                    now: sidecar.now,
                    done_this_turn: vec![],
                    blockers: sidecar.blockers,
                    decisions: sidecar.decisions,
                    findings: sidecar.findings,
                    next_steps: sidecar.next_steps,
                    affected_targets: sidecar.affected_targets,
                    source_refs: vec![],
                    urgency_level,
                    recommended_response_level,
                };

                vec![Action::GuardedContextInjection {
                    session_key: event.tmux_session.clone(),
                    project_path,
                    commit,
                    evidence,
                    proposal,
                    feedback_packet: Some(feedback_packet),
                }]
            }
            _ => {
                // Blocked — log and emit the commit for audit (via the action)
                let summary = commit.attestation.summary.clone();
                vec![
                    Action::GuardedContextInjection {
                        session_key: event.tmux_session.clone(),
                        project_path,
                        commit,
                        evidence,
                        proposal,
                        feedback_packet: None,
                    },
                    Action::Log {
                        message: format!(
                            "Context injection blocked for {}: {}",
                            event.tmux_session, summary
                        ),
                    },
                ]
            }
        }
    }
}
