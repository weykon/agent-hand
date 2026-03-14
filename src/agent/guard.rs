//! Guarded context-injection pipeline.
//!
//! Every context injection goes through: Proposal → Evidence → Guard → Commit.
//! The guard is a pure, deterministic function — no side effects, no I/O.
//! All decisions are recorded for audit.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::ContextBridgeConfig;

// ── Risk & Response Levels ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseLevel {
    L0Ignore,
    L1RecordOnly,
    L2SelfInject,
    L3CrossSessionInject,
    L4HumanConfirm,
}

// ── Injection Scope ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InjectionScope {
    Off,
    SelfOnly,
    SameGroup,
    ConfirmedRelations,
}

impl InjectionScope {
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "off" => Self::Off,
            "self_only" => Self::SelfOnly,
            "same_group" => Self::SameGroup,
            "confirmed_relations" => Self::ConfirmedRelations,
            _ => Self::SelfOnly,
        }
    }
}

// ── Proposal ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalKind {
    InjectContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub trace_id: String,
    pub session_key: String,
    pub source_session_id: String,
    pub kind: ProposalKind,
    pub project_path: PathBuf,
    pub scope: InjectionScope,
    pub risk: RiskLevel,
    pub created_at_ms: u64,
}

// ── Evidence ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceKind {
    SessionState,
    ProgressLog,
    EventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub trace_id: String,
    pub session_key: String,
    pub kind: EvidenceKind,
    pub captured_at_ms: u64,
    pub data: serde_json::Value,
}

// ── Guard Attestation ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardCheck {
    pub name: String,
    pub passed: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub passed: bool,
    pub summary: String,
    pub checks: Vec<GuardCheck>,
}

// ── Guard Decision & Commit ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuardDecision {
    Approve,
    NeedsEvidence,
    Block,
    Downgrade,
    NeedsHuman,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardedCommit {
    pub commit_id: String,
    pub trace_id: String,
    pub session_key: String,
    pub proposal_id: String,
    pub decision: GuardDecision,
    pub attestation: Attestation,
    pub created_at_ms: u64,
}

// ── Feedback Packet ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackPacket {
    pub packet_id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub created_at_ms: u64,
    pub goal: Option<String>,
    pub now: Option<String>,
    pub done_this_turn: Vec<String>,
    pub blockers: Vec<String>,
    pub decisions: Vec<String>,
    pub findings: Vec<String>,
    pub next_steps: Vec<String>,
    pub affected_targets: Vec<String>,
    pub source_refs: Vec<String>,
    pub urgency_level: RiskLevel,
    pub recommended_response_level: ResponseLevel,
}

// ── Sidecar Feedback ────────────────────────────────────────────────

/// JSON schema that external agents write to `{runtime_dir}/sidecar/{session_key}.json`.
///
/// All fields are optional via `#[serde(default)]`. An empty `{}` file
/// produces `SidecarFeedback::default()` — identical to the pre-sidecar behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SidecarFeedback {
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub now: Option<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub findings: Vec<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub affected_targets: Vec<String>,
    #[serde(default = "default_urgency", deserialize_with = "deserialize_urgency")]
    pub urgency: String,
}

fn default_urgency() -> String {
    "low".to_string()
}

fn deserialize_urgency<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer).unwrap_or_default();
    match s.to_lowercase().as_str() {
        "low" | "medium" | "high" | "critical" => Ok(s.to_lowercase()),
        _ => Ok("low".to_string()),
    }
}

/// Map a sidecar urgency string to the pipeline's RiskLevel.
pub fn parse_urgency(s: &str) -> RiskLevel {
    match s.to_lowercase().as_str() {
        "critical" => RiskLevel::Critical,
        "high" => RiskLevel::High,
        "medium" => RiskLevel::Medium,
        _ => RiskLevel::Low,
    }
}

// ── ID Generation ───────────────────────────────────────────────────

pub fn short_id() -> String {
    uuid::Uuid::new_v4().to_string()[..12].to_string()
}

// ── Guard Function ──────────────────────────────────────────────────

/// Deterministic guard: runs 8 checks, returns (decision, attestation).
///
/// All checks must pass for Approve. Any failure → Block.
/// No I/O, no side effects — pure function.
pub fn run_guard(
    proposal: &Proposal,
    evidence: &[EvidenceRecord],
    config: &ContextBridgeConfig,
    last_injection_ts: Option<f64>,
    current_ts: f64,
    already_injected_this_turn: bool,
) -> (GuardDecision, Attestation) {
    let mut checks = Vec::with_capacity(8);

    // 1. Bridge enabled
    checks.push(GuardCheck {
        name: "bridge_enabled".into(),
        passed: config.enabled,
        detail: if config.enabled {
            None
        } else {
            Some("context bridge is disabled in config".into())
        },
    });

    // 2. Proposal target exists (session_key non-empty)
    let target_ok = !proposal.session_key.is_empty();
    checks.push(GuardCheck {
        name: "proposal_target_exists".into(),
        passed: target_ok,
        detail: if target_ok {
            None
        } else {
            Some("session_key is empty".into())
        },
    });

    // 3. Project path exists (non-empty)
    let path_ok = !proposal.project_path.as_os_str().is_empty();
    checks.push(GuardCheck {
        name: "project_path_exists".into(),
        passed: path_ok,
        detail: if path_ok {
            None
        } else {
            Some("project_path is empty".into())
        },
    });

    // 4. Evidence exists
    let evidence_ok = !evidence.is_empty();
    checks.push(GuardCheck {
        name: "evidence_exists".into(),
        passed: evidence_ok,
        detail: if evidence_ok {
            Some(format!("{} evidence record(s)", evidence.len()))
        } else {
            Some("no evidence provided".into())
        },
    });

    // 5. Scope is self_only (MVP restriction)
    let scope_ok = proposal.scope == InjectionScope::SelfOnly;
    checks.push(GuardCheck {
        name: "scope_is_self_only".into(),
        passed: scope_ok,
        detail: if scope_ok {
            None
        } else {
            Some(format!("scope is {:?}, only SelfOnly allowed in MVP", proposal.scope))
        },
    });

    // 6. Cooldown satisfied
    let cooldown_ok = match last_injection_ts {
        Some(last_ts) => (current_ts - last_ts) >= config.cooldown_secs as f64,
        None => true, // No prior injection — cooldown trivially satisfied
    };
    checks.push(GuardCheck {
        name: "cooldown_satisfied".into(),
        passed: cooldown_ok,
        detail: if cooldown_ok {
            None
        } else {
            let elapsed = current_ts - last_injection_ts.unwrap_or(0.0);
            Some(format!(
                "cooldown not met: {:.1}s elapsed, {}s required",
                elapsed, config.cooldown_secs
            ))
        },
    });

    // 7. Budget configured
    let budget_ok = config.max_lines > 0 && config.max_total_chars > 0;
    checks.push(GuardCheck {
        name: "budget_configured".into(),
        passed: budget_ok,
        detail: if budget_ok {
            Some(format!(
                "max_lines={}, max_total_chars={}",
                config.max_lines, config.max_total_chars
            ))
        } else {
            Some("max_lines or max_total_chars is 0".into())
        },
    });

    // 8. No duplicate injection this turn
    let no_dup = !already_injected_this_turn;
    checks.push(GuardCheck {
        name: "no_duplicate_injection".into(),
        passed: no_dup,
        detail: if no_dup {
            None
        } else {
            Some("already injected for this session in current event".into())
        },
    });

    // Decision: all pass → Approve, any fail → Block
    let all_passed = checks.iter().all(|c| c.passed);
    let failed: Vec<&str> = checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.name.as_str())
        .collect();

    let (decision, summary) = if all_passed {
        (
            GuardDecision::Approve,
            "all 8 checks passed — context injection approved".into(),
        )
    } else {
        (
            GuardDecision::Block,
            format!("blocked: failed checks [{}]", failed.join(", ")),
        )
    };

    let attestation = Attestation {
        passed: all_passed,
        summary,
        checks,
    };

    (decision, attestation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ContextBridgeConfig {
        ContextBridgeConfig::default()
    }

    fn make_proposal(trace_id: &str) -> Proposal {
        Proposal {
            id: short_id(),
            trace_id: trace_id.into(),
            session_key: "test_session".into(),
            source_session_id: "sid-123".into(),
            kind: ProposalKind::InjectContext,
            project_path: PathBuf::from("/tmp/proj"),
            scope: InjectionScope::SelfOnly,
            risk: RiskLevel::Low,
            created_at_ms: 1700000000000,
        }
    }

    fn make_evidence(trace_id: &str) -> Vec<EvidenceRecord> {
        vec![EvidenceRecord {
            id: short_id(),
            trace_id: trace_id.into(),
            session_key: "test_session".into(),
            kind: EvidenceKind::SessionState,
            captured_at_ms: 1700000000000,
            data: serde_json::json!({"status": "running"}),
        }]
    }

    #[test]
    fn guard_approves_when_all_checks_pass() {
        let config = default_config();
        let proposal = make_proposal("trace-1");
        let evidence = make_evidence("trace-1");

        let (decision, attestation) =
            run_guard(&proposal, &evidence, &config, None, 1700000000.0, false);

        assert!(matches!(decision, GuardDecision::Approve));
        assert!(attestation.passed);
        assert_eq!(attestation.checks.len(), 8);
        assert!(attestation.checks.iter().all(|c| c.passed));
    }

    #[test]
    fn guard_blocks_when_bridge_disabled() {
        let mut config = default_config();
        config.enabled = false;
        let proposal = make_proposal("trace-2");
        let evidence = make_evidence("trace-2");

        let (decision, attestation) =
            run_guard(&proposal, &evidence, &config, None, 1700000000.0, false);

        assert!(matches!(decision, GuardDecision::Block));
        assert!(!attestation.passed);
        assert!(!attestation.checks[0].passed);
    }

    #[test]
    fn guard_blocks_when_no_evidence() {
        let config = default_config();
        let proposal = make_proposal("trace-3");

        let (decision, attestation) =
            run_guard(&proposal, &[], &config, None, 1700000000.0, false);

        assert!(matches!(decision, GuardDecision::Block));
        assert!(attestation.summary.contains("evidence_exists"));
    }

    #[test]
    fn guard_blocks_on_cooldown_violation() {
        let config = default_config();
        let proposal = make_proposal("trace-4");
        let evidence = make_evidence("trace-4");

        // Last injection was 2 seconds ago, cooldown is 5 seconds
        let (decision, _) = run_guard(
            &proposal,
            &evidence,
            &config,
            Some(1700000000.0 - 2.0),
            1700000000.0,
            false,
        );

        assert!(matches!(decision, GuardDecision::Block));
    }

    #[test]
    fn guard_blocks_duplicate_injection() {
        let config = default_config();
        let proposal = make_proposal("trace-5");
        let evidence = make_evidence("trace-5");

        let (decision, attestation) =
            run_guard(&proposal, &evidence, &config, None, 1700000000.0, true);

        assert!(matches!(decision, GuardDecision::Block));
        assert!(attestation.summary.contains("no_duplicate_injection"));
    }

    #[test]
    fn guard_approves_after_cooldown_elapsed() {
        let config = default_config();
        let proposal = make_proposal("trace-6");
        let evidence = make_evidence("trace-6");

        // Last injection was 10 seconds ago, cooldown is 5 seconds
        let (decision, _) = run_guard(
            &proposal,
            &evidence,
            &config,
            Some(1700000000.0 - 10.0),
            1700000000.0,
            false,
        );

        assert!(matches!(decision, GuardDecision::Approve));
    }

    #[test]
    fn guard_blocks_non_self_scope() {
        let config = default_config();
        let mut proposal = make_proposal("trace-7");
        proposal.scope = InjectionScope::SameGroup;
        let evidence = make_evidence("trace-7");

        let (decision, attestation) =
            run_guard(&proposal, &evidence, &config, None, 1700000000.0, false);

        assert!(matches!(decision, GuardDecision::Block));
        assert!(attestation.summary.contains("scope_is_self_only"));
    }
}
