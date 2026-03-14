//! Projection/view-model layer over shared runtime state.
//!
//! These structs are intentionally renderer-agnostic. They provide the first
//! explicit projection layer for relationship / scheduler / evidence / workflow views.
//!
//! Design source: specs/23 (canvas/workflow views), specs/27.

use serde::{Deserialize, Serialize};

use crate::session::{Instance, Relationship};

use super::guard::{EvidenceRecord, FeedbackPacket, GuardedCommit, RiskLevel};
use super::scheduler::{SchedulerRecord, SchedulerState};

// ── Relationship Projection ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipNodeView {
    pub session_id: String,
    pub title: String,
    pub group_path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipEdgeView {
    pub relationship_id: String,
    pub source_session_id: String,
    pub target_session_id: String,
    pub edge_type: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipViewModel {
    pub nodes: Vec<RelationshipNodeView>,
    pub edges: Vec<RelationshipEdgeView>,
}

pub fn build_relationship_view_model(
    sessions: &[Instance],
    relationships: &[Relationship],
) -> RelationshipViewModel {
    let nodes = sessions
        .iter()
        .map(|s| RelationshipNodeView {
            session_id: s.id.clone(),
            title: s.title.clone(),
            group_path: s.group_path.clone(),
            status: format!("{:?}", s.status).to_lowercase(),
        })
        .collect();

    let edges = relationships
        .iter()
        .map(|r| RelationshipEdgeView {
            relationship_id: r.id.clone(),
            source_session_id: r.session_a_id.clone(),
            target_session_id: r.session_b_id.clone(),
            edge_type: r.relation_type.to_string(),
            label: r.label.clone(),
        })
        .collect();

    RelationshipViewModel { nodes, edges }
}

// ── Scheduler Projection ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerViewModel {
    pub pending_coordination: Vec<SchedulerRecord>,
    pub review_queue: Vec<SchedulerRecord>,
    pub proposed_followups: Vec<SchedulerRecord>,
}

pub fn build_scheduler_view_model(state: &SchedulerState) -> SchedulerViewModel {
    SchedulerViewModel {
        pending_coordination: state.pending_coordination.clone(),
        review_queue: state.review_queue.clone(),
        proposed_followups: state.proposed_followups.clone(),
    }
}

// ── Evidence Projection ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceDecisionView {
    pub commit_id: String,
    pub trace_id: String,
    pub session_key: String,
    pub decision: String,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceViewModel {
    pub decisions: Vec<EvidenceDecisionView>,
}

pub fn build_evidence_view_model(
    commits: &[GuardedCommit],
    evidence: &[EvidenceRecord],
) -> EvidenceViewModel {
    let decisions = commits
        .iter()
        .map(|commit| {
            let passed_checks = commit
                .attestation
                .checks
                .iter()
                .filter(|c| c.passed)
                .count();
            let failed_checks = commit.attestation.checks.len() - passed_checks;
            let evidence_count = evidence
                .iter()
                .filter(|ev| ev.trace_id == commit.trace_id)
                .count();
            EvidenceDecisionView {
                commit_id: commit.commit_id.clone(),
                trace_id: commit.trace_id.clone(),
                session_key: commit.session_key.clone(),
                decision: format!("{:?}", commit.decision),
                passed_checks,
                failed_checks,
                evidence_count,
            }
        })
        .collect();

    EvidenceViewModel { decisions }
}

// ── Workflow Projection ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepView {
    pub session_id: String,
    pub goal: Option<String>,
    pub now: Option<String>,
    pub blockers: Vec<String>,
    pub next_steps: Vec<String>,
    pub urgency_level: RiskLevel,
    pub scheduler_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowViewModel {
    pub steps: Vec<WorkflowStepView>,
}

pub fn build_workflow_view_model(
    packets: &[FeedbackPacket],
    scheduler_state: &SchedulerState,
) -> WorkflowViewModel {
    let steps = packets
        .iter()
        .map(|packet| {
            let scheduler_state_label = if scheduler_state
                .review_queue
                .iter()
                .any(|r| r.trace_id == packet.trace_id)
            {
                Some("review_queue".to_string())
            } else if scheduler_state
                .proposed_followups
                .iter()
                .any(|r| r.trace_id == packet.trace_id)
            {
                Some("proposed_followup".to_string())
            } else if scheduler_state
                .pending_coordination
                .iter()
                .any(|r| r.trace_id == packet.trace_id)
            {
                Some("pending_coordination".to_string())
            } else {
                None
            };

            WorkflowStepView {
                session_id: packet.source_session_id.clone(),
                goal: packet.goal.clone(),
                now: packet.now.clone(),
                blockers: packet.blockers.clone(),
                next_steps: packet.next_steps.clone(),
                urgency_level: packet.urgency_level.clone(),
                scheduler_state: scheduler_state_label,
            }
        })
        .collect();

    WorkflowViewModel { steps }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::consumers::SchedulerDisposition;
    use crate::agent::scheduler::{SchedulerRecord, SchedulerState};
    use crate::session::{Instance, Relationship, RelationType, Status};
    use std::path::PathBuf;

    fn session(id: &str, title: &str, group: &str) -> Instance {
        let mut inst = Instance::with_group(
            title.to_string(),
            PathBuf::from(format!("/tmp/{}", title)),
            group.to_string(),
        );
        inst.id = id.to_string();
        inst.status = Status::Idle;
        inst
    }

    fn packet(trace_id: &str, sid: &str) -> FeedbackPacket {
        FeedbackPacket {
            packet_id: format!("pkt-{}", sid),
            trace_id: trace_id.to_string(),
            source_session_id: sid.to_string(),
            created_at_ms: 1700000000000,
            goal: Some("goal".to_string()),
            now: Some("now".to_string()),
            done_this_turn: vec![],
            blockers: vec!["blocker".to_string()],
            decisions: vec![],
            findings: vec![],
            next_steps: vec!["next".to_string()],
            affected_targets: vec![],
            source_refs: vec!["ref-1".to_string()],
            urgency_level: RiskLevel::High,
            recommended_response_level: crate::agent::guard::ResponseLevel::L2SelfInject,
        }
    }

    #[test]
    fn builds_relationship_view_model() {
        let sessions = vec![session("a", "A", "g"), session("b", "B", "g")];
        let rel = Relationship::new(RelationType::Dependency, "a".into(), "b".into());
        let view = build_relationship_view_model(&sessions, &[rel]);
        assert_eq!(view.nodes.len(), 2);
        assert_eq!(view.edges.len(), 1);
        assert_eq!(view.edges[0].edge_type, "dependency");
    }

    #[test]
    fn builds_scheduler_view_model() {
        let state = SchedulerState {
            pending_coordination: vec![SchedulerRecord {
                id: "r1".to_string(),
                trace_id: "t1".to_string(),
                source_session_id: "a".to_string(),
                target_session_ids: vec!["b".to_string()],
                disposition: SchedulerDisposition::PendingCoordination,
                reason: "reason".to_string(),
                urgency_level: RiskLevel::Medium,
                created_at_ms: 1,
            }],
            review_queue: vec![],
            proposed_followups: vec![],
        };
        let view = build_scheduler_view_model(&state);
        assert_eq!(view.pending_coordination.len(), 1);
    }

    #[test]
    fn builds_evidence_view_model() {
        let commit = GuardedCommit {
            commit_id: "c1".to_string(),
            trace_id: "t1".to_string(),
            session_key: "session-a".to_string(),
            proposal_id: "p1".to_string(),
            decision: crate::agent::guard::GuardDecision::Approve,
            attestation: crate::agent::guard::Attestation {
                passed: false,
                summary: "summary".to_string(),
                checks: vec![
                    crate::agent::guard::GuardCheck {
                        name: "a".to_string(),
                        passed: true,
                        detail: None,
                    },
                    crate::agent::guard::GuardCheck {
                        name: "b".to_string(),
                        passed: false,
                        detail: None,
                    },
                ],
            },
            created_at_ms: 1,
        };
        let evidence = vec![
            EvidenceRecord {
                id: "e1".to_string(),
                trace_id: "t1".to_string(),
                session_key: "session-a".to_string(),
                kind: crate::agent::guard::EvidenceKind::SessionState,
                captured_at_ms: 1,
                data: serde_json::json!({}),
            },
            EvidenceRecord {
                id: "e2".to_string(),
                trace_id: "t1".to_string(),
                session_key: "session-a".to_string(),
                kind: crate::agent::guard::EvidenceKind::EventMetadata,
                captured_at_ms: 1,
                data: serde_json::json!({}),
            },
        ];
        let view = build_evidence_view_model(&[commit], &evidence);
        assert_eq!(view.decisions.len(), 1);
        assert_eq!(view.decisions[0].passed_checks, 1);
        assert_eq!(view.decisions[0].failed_checks, 1);
        assert_eq!(view.decisions[0].evidence_count, 2);
    }

    #[test]
    fn builds_workflow_view_model() {
        let packet = packet("trace-a", "session-a");
        let state = SchedulerState {
            pending_coordination: vec![],
            review_queue: vec![SchedulerRecord {
                id: "r1".to_string(),
                trace_id: "trace-a".to_string(),
                source_session_id: "session-a".to_string(),
                target_session_ids: vec!["session-b".to_string()],
                disposition: SchedulerDisposition::NeedsHumanReview,
                reason: "reason".to_string(),
                urgency_level: RiskLevel::High,
                created_at_ms: 1,
            }],
            proposed_followups: vec![],
        };
        let view = build_workflow_view_model(&[packet], &state);
        assert_eq!(view.steps.len(), 1);
        assert_eq!(view.steps[0].scheduler_state.as_deref(), Some("review_queue"));
    }
}
