//! Scheduler-side bounded state derived from normalized scheduler outputs.
//!
//! This module deliberately does NOT execute scheduling actions.
//! It only converts normalized outputs into bounded scheduler state records.
//!
//! Design sources: specs/21-22 (scheduler normalized outputs),
//! specs/27 (second-round development brief).

use serde::{Deserialize, Serialize};

use super::consumers::{SchedulerDisposition, SchedulerNormalizedOutput};
use super::guard::RiskLevel;

/// A scheduler-side record that can later be consumed by deterministic scheduler logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerRecord {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub disposition: SchedulerDisposition,
    pub reason: String,
    pub urgency_level: RiskLevel,
    pub created_at_ms: u64,
}

/// Bounded scheduler-side state derived from normalized outputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulerState {
    pub pending_coordination: Vec<SchedulerRecord>,
    pub review_queue: Vec<SchedulerRecord>,
    pub proposed_followups: Vec<SchedulerRecord>,
}

/// Status of a follow-up proposal in its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal is waiting for consumption (default state after generation).
    Pending,
    /// Proposal has been accepted by human review or automated policy.
    Accepted,
    /// Proposal has been rejected (with reason).
    Rejected,
}

impl Default for ProposalStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A bounded formal follow-up proposal derived from scheduler-side state.
///
/// This is still not a live scheduling action. It is the first execution-ready
/// scheduling-side record that can later be consumed by deterministic runtime logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowupProposalRecord {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub proposal_kind: String,
    pub reason: String,
    pub urgency_level: RiskLevel,
    pub created_at_ms: u64,
    /// Lifecycle status of this proposal.
    #[serde(default)]
    pub status: ProposalStatus,
    /// Reason for rejection (only set when status == Rejected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

/// Build a bounded scheduler state from normalized outputs.
///
/// Mapping:
/// - Ignore / RecordOnly -> not included in scheduler state
/// - PendingCoordination -> pending_coordination
/// - NeedsHumanReview -> review_queue
/// - ProposeFollowup -> proposed_followups
pub fn build_scheduler_state(
    outputs: &[SchedulerNormalizedOutput],
    created_at_ms: u64,
) -> SchedulerState {
    let mut state = SchedulerState::default();

    for output in outputs {
        let record = SchedulerRecord {
            id: output.id.clone(),
            trace_id: output.trace_id.clone(),
            source_session_id: output.source_session_id.clone(),
            target_session_ids: output.target_session_ids.clone(),
            disposition: output.disposition.clone(),
            reason: output.reason.clone(),
            urgency_level: output.urgency_level.clone(),
            created_at_ms,
        };

        match output.disposition {
            SchedulerDisposition::Ignore | SchedulerDisposition::RecordOnly => {}
            SchedulerDisposition::PendingCoordination => state.pending_coordination.push(record),
            SchedulerDisposition::NeedsHumanReview => state.review_queue.push(record),
            SchedulerDisposition::ProposeFollowup => state.proposed_followups.push(record),
        }
    }

    state
}

/// Build bounded follow-up proposal records from scheduler state.
///
/// V1 rule:
/// - only `proposed_followups` become follow-up proposal records
/// - `pending_coordination` remains scheduler-side state only
/// - `review_queue` remains human-facing review state only
pub fn build_followup_proposals(
    state: &SchedulerState,
    max_records: usize,
) -> Vec<FollowupProposalRecord> {
    state
        .proposed_followups
        .iter()
        .take(max_records)
        .map(|record| FollowupProposalRecord {
            id: record.id.clone(),
            trace_id: record.trace_id.clone(),
            source_session_id: record.source_session_id.clone(),
            target_session_ids: record.target_session_ids.clone(),
            proposal_kind: "followup".to_string(),
            reason: record.reason.clone(),
            urgency_level: record.urgency_level.clone(),
            created_at_ms: record.created_at_ms,
            status: ProposalStatus::Pending,
            rejection_reason: None,
        })
        .collect()
}

/// Accept a proposal by ID. Returns true if the proposal was found and updated.
pub fn accept_proposal(proposals: &mut [FollowupProposalRecord], proposal_id: &str) -> bool {
    if let Some(p) = proposals.iter_mut().find(|p| p.id == proposal_id && p.status == ProposalStatus::Pending) {
        p.status = ProposalStatus::Accepted;
        true
    } else {
        false
    }
}

/// Reject a proposal by ID with a reason. Returns true if the proposal was found and updated.
pub fn reject_proposal(proposals: &mut [FollowupProposalRecord], proposal_id: &str, reason: &str) -> bool {
    if let Some(p) = proposals.iter_mut().find(|p| p.id == proposal_id && p.status == ProposalStatus::Pending) {
        p.status = ProposalStatus::Rejected;
        p.rejection_reason = Some(reason.to_string());
        true
    } else {
        false
    }
}

/// Load proposals from a JSON snapshot file.
pub fn load_proposals(path: &std::path::Path) -> Vec<FollowupProposalRecord> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save proposals to a JSON snapshot file.
pub fn save_proposals(path: &std::path::Path, proposals: &[FollowupProposalRecord]) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(proposals)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

/// Count proposals by status.
pub fn proposal_counts(proposals: &[FollowupProposalRecord]) -> (usize, usize, usize) {
    let pending = proposals.iter().filter(|p| p.status == ProposalStatus::Pending).count();
    let accepted = proposals.iter().filter(|p| p.status == ProposalStatus::Accepted).count();
    let rejected = proposals.iter().filter(|p| p.status == ProposalStatus::Rejected).count();
    (pending, accepted, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(id: &str, disposition: SchedulerDisposition) -> SchedulerNormalizedOutput {
        SchedulerNormalizedOutput {
            id: id.to_string(),
            trace_id: "trace-1".to_string(),
            source_session_id: "session-a".to_string(),
            target_session_ids: vec!["session-b".to_string()],
            disposition,
            reason: "test reason".to_string(),
            urgency_level: RiskLevel::High,
        }
    }

    #[test]
    fn build_scheduler_state_routes_outputs_by_disposition() {
        let outputs = vec![
            output("ignore", SchedulerDisposition::Ignore),
            output("record", SchedulerDisposition::RecordOnly),
            output("pending", SchedulerDisposition::PendingCoordination),
            output("review", SchedulerDisposition::NeedsHumanReview),
            output("followup", SchedulerDisposition::ProposeFollowup),
        ];

        let state = build_scheduler_state(&outputs, 1700000000000);
        assert_eq!(state.pending_coordination.len(), 1);
        assert_eq!(state.review_queue.len(), 1);
        assert_eq!(state.proposed_followups.len(), 1);
        assert_eq!(state.pending_coordination[0].id, "pending");
        assert_eq!(state.review_queue[0].id, "review");
        assert_eq!(state.proposed_followups[0].id, "followup");
    }

    #[test]
    fn build_scheduler_state_preserves_traceability() {
        let outputs = vec![output("pending", SchedulerDisposition::PendingCoordination)];
        let state = build_scheduler_state(&outputs, 1700000000000);
        let record = &state.pending_coordination[0];
        assert_eq!(record.trace_id, "trace-1");
        assert_eq!(record.source_session_id, "session-a");
        assert_eq!(record.target_session_ids, vec!["session-b".to_string()]);
        assert_eq!(record.created_at_ms, 1700000000000);
    }

    #[test]
    fn build_followup_proposals_only_uses_proposed_followups() {
        let outputs = vec![
            output("pending", SchedulerDisposition::PendingCoordination),
            output("review", SchedulerDisposition::NeedsHumanReview),
            output("followup", SchedulerDisposition::ProposeFollowup),
        ];

        let state = build_scheduler_state(&outputs, 1700000000000);
        let proposals = build_followup_proposals(&state, 10);

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, "followup");
        assert_eq!(proposals[0].proposal_kind, "followup");
    }

    #[test]
    fn build_followup_proposals_respects_bound() {
        let outputs = vec![
            output("followup-a", SchedulerDisposition::ProposeFollowup),
            output("followup-b", SchedulerDisposition::ProposeFollowup),
        ];

        let state = build_scheduler_state(&outputs, 1700000000000);
        let proposals = build_followup_proposals(&state, 1);

        assert_eq!(proposals.len(), 1);
    }

    #[test]
    fn accept_proposal_transitions_pending_to_accepted() {
        let outputs = vec![output("f1", SchedulerDisposition::ProposeFollowup)];
        let state = build_scheduler_state(&outputs, 1);
        let mut proposals = build_followup_proposals(&state, 10);
        assert_eq!(proposals[0].status, ProposalStatus::Pending);

        let ok = accept_proposal(&mut proposals, "f1");
        assert!(ok);
        assert_eq!(proposals[0].status, ProposalStatus::Accepted);
    }

    #[test]
    fn reject_proposal_transitions_pending_to_rejected_with_reason() {
        let outputs = vec![output("f2", SchedulerDisposition::ProposeFollowup)];
        let state = build_scheduler_state(&outputs, 1);
        let mut proposals = build_followup_proposals(&state, 10);

        let ok = reject_proposal(&mut proposals, "f2", "not relevant");
        assert!(ok);
        assert_eq!(proposals[0].status, ProposalStatus::Rejected);
        assert_eq!(proposals[0].rejection_reason.as_deref(), Some("not relevant"));
    }

    #[test]
    fn cannot_accept_already_rejected_proposal() {
        let outputs = vec![output("f3", SchedulerDisposition::ProposeFollowup)];
        let state = build_scheduler_state(&outputs, 1);
        let mut proposals = build_followup_proposals(&state, 10);

        reject_proposal(&mut proposals, "f3", "bad");
        let ok = accept_proposal(&mut proposals, "f3");
        assert!(!ok); // already rejected, can't accept
        assert_eq!(proposals[0].status, ProposalStatus::Rejected);
    }

    #[test]
    fn proposal_counts_are_correct() {
        let outputs = vec![
            output("a", SchedulerDisposition::ProposeFollowup),
            output("b", SchedulerDisposition::ProposeFollowup),
            output("c", SchedulerDisposition::ProposeFollowup),
        ];
        let state = build_scheduler_state(&outputs, 1);
        let mut proposals = build_followup_proposals(&state, 10);

        accept_proposal(&mut proposals, "a");
        reject_proposal(&mut proposals, "c", "no");

        let (pending, accepted, rejected) = proposal_counts(&proposals);
        assert_eq!(pending, 1);
        assert_eq!(accepted, 1);
        assert_eq!(rejected, 1);
    }

    #[test]
    fn proposal_roundtrip_serialization() {
        let outputs = vec![output("f4", SchedulerDisposition::ProposeFollowup)];
        let state = build_scheduler_state(&outputs, 1);
        let mut proposals = build_followup_proposals(&state, 10);
        accept_proposal(&mut proposals, "f4");

        let json = serde_json::to_string(&proposals).unwrap();
        let loaded: Vec<FollowupProposalRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded[0].status, ProposalStatus::Accepted);
        assert_eq!(loaded[0].id, "f4");
    }
}
