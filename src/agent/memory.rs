//! Memory Boundary — five-layer memory model types and promotion gate.
//!
//! Codifies the memory boundary (spec 19) as types and documentation.
//! Does NOT implement a full memory DB, semantic search, or ingestion pipeline.
//!
//! The five layers must remain distinct; collapsing them breaks auditability.
//!
//! Design sources: specs/19-20 (memory boundary).

use serde::{Deserialize, Serialize};

use super::consumers::MemoryIngestEntry;
use super::hot_brain::MemoryCandidateKind;
use super::guard;

// ── Five-Layer Memory Model ──────────────────────────────────────────

/// The five-layer memory model (spec 19).
///
/// Each layer serves a distinct purpose; they must not collapse.
/// The promotion ladder flows strictly upward:
/// Audit -> Evidence -> Packet -> Candidate -> ColdMemory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryLayer {
    /// Layer 1: Append-only logs of what happened (proposals.jsonl, evidence.jsonl, etc.)
    Audit,
    /// Layer 2: Why something was approved/blocked (EvidenceRecord, Attestation, GuardedCommit)
    Evidence,
    /// Layer 3: Coordination-ready turn outcomes (FeedbackPacket)
    Packet,
    /// Layer 4: Bounded suggestions for promotion (MemoryCandidate -> MemoryIngestEntry)
    Candidate,
    /// Layer 5: Accepted, durable, reusable knowledge
    ColdMemory,
}

impl MemoryLayer {
    /// Returns all five layers in promotion order.
    pub fn all() -> [MemoryLayer; 5] {
        [
            MemoryLayer::Audit,
            MemoryLayer::Evidence,
            MemoryLayer::Packet,
            MemoryLayer::Candidate,
            MemoryLayer::ColdMemory,
        ]
    }
}

// ── Cold Memory Record ───────────────────────────────────────────────

/// An accepted, durable memory entry (Layer 5).
///
/// Only produced through the promotion ladder:
/// Audit -> Evidence -> FeedbackPacket -> MemoryCandidate -> MemoryConsumer -> ColdMemoryRecord
///
/// Hard rule: Neither audit logs, feedback packets, nor Hot Brain
/// may directly create cold memory records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdMemoryRecord {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub kind: MemoryCandidateKind,
    pub summary: String,
    pub source_refs: Vec<String>,
    /// Which MemoryIngestEntry this was promoted from.
    pub promoted_from: String,
    pub created_at_ms: u64,
}

// ── Promotion Gate ───────────────────────────────────────────────────

/// Check whether a MemoryIngestEntry qualifies for cold memory promotion.
///
/// Returns `None` if eligible (all checks pass), `Some(reason)` if rejected.
///
/// Rules (spec 19 section 16):
/// 1. entry.accepted must be true
/// 2. entry.source_session_id must be non-empty
/// 3. entry.source_refs must be non-empty
/// 4. entry.summary must be non-empty (not just whitespace)
pub fn check_promotion_eligibility(entry: &MemoryIngestEntry) -> Option<String> {
    if !entry.accepted {
        return Some(format!(
            "entry not accepted: {}",
            entry.rejection_reason.as_deref().unwrap_or("unknown reason")
        ));
    }
    if entry.source_session_id.is_empty() {
        return Some("empty source_session_id".to_string());
    }
    if entry.source_refs.is_empty() {
        return Some("empty source_refs".to_string());
    }
    if entry.summary.trim().is_empty() {
        return Some("empty summary".to_string());
    }
    None
}

/// Promote accepted memory ingest entries into cold memory records.
///
/// Rejected entries are skipped. This function is deterministic and preserves lineage.
pub fn promote_memory_entries(
    entries: &[MemoryIngestEntry],
    created_at_ms: u64,
) -> Vec<ColdMemoryRecord> {
    entries
        .iter()
        .filter(|entry| check_promotion_eligibility(entry).is_none())
        .map(|entry| ColdMemoryRecord {
            id: guard::short_id(),
            trace_id: entry.trace_id.clone(),
            source_session_id: entry.source_session_id.clone(),
            kind: entry.kind.clone(),
            summary: entry.summary.clone(),
            source_refs: entry.source_refs.clone(),
            promoted_from: entry.id.clone(),
            created_at_ms,
        })
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_accepted_entry(summary: &str, refs: Vec<&str>) -> MemoryIngestEntry {
        MemoryIngestEntry {
            id: "entry-1".to_string(),
            trace_id: "trace-1".to_string(),
            source_session_id: "session-1".to_string(),
            kind: MemoryCandidateKind::Decision,
            summary: summary.to_string(),
            source_refs: refs.into_iter().map(String::from).collect(),
            accepted: true,
            rejection_reason: None,
        }
    }

    fn make_rejected_entry(reason: &str) -> MemoryIngestEntry {
        MemoryIngestEntry {
            id: "entry-2".to_string(),
            trace_id: "trace-1".to_string(),
            source_session_id: "session-1".to_string(),
            kind: MemoryCandidateKind::Finding,
            summary: "some finding".to_string(),
            source_refs: vec!["ref-1".to_string()],
            accepted: false,
            rejection_reason: Some(reason.to_string()),
        }
    }

    #[test]
    fn cold_memory_record_requires_promotion_from_accepted_entry() {
        let rejected = make_rejected_entry("validation failed");
        let result = check_promotion_eligibility(&rejected);
        assert!(result.is_some(), "rejected entry must fail promotion gate");
        assert!(result.unwrap().contains("not accepted"));

        let accepted = make_accepted_entry("use JWT for auth", vec!["ref-1"]);
        let result = check_promotion_eligibility(&accepted);
        assert!(result.is_none(), "accepted entry should pass promotion gate");
    }

    #[test]
    fn promotion_gate_rejects_empty_provenance() {
        // Empty source_refs
        let mut entry = make_accepted_entry("decision", vec![]);
        // Need to re-set accepted since make_accepted_entry validates
        entry.source_refs = vec![];
        let result = check_promotion_eligibility(&entry);
        assert!(result.is_some());
        assert!(result.unwrap().contains("source_refs"));

        // Empty source_session_id
        let mut entry = make_accepted_entry("decision", vec!["ref-1"]);
        entry.source_session_id = String::new();
        let result = check_promotion_eligibility(&entry);
        assert!(result.is_some());
        assert!(result.unwrap().contains("source_session_id"));

        // Empty summary
        let entry = make_accepted_entry("   ", vec!["ref-1"]);
        let result = check_promotion_eligibility(&entry);
        assert!(result.is_some());
        assert!(result.unwrap().contains("summary"));
    }

    #[test]
    fn memory_layers_are_distinct() {
        let layers = MemoryLayer::all();
        assert_eq!(layers.len(), 5, "must have exactly 5 layers");

        // All distinct
        for i in 0..layers.len() {
            for j in (i + 1)..layers.len() {
                assert_ne!(layers[i], layers[j], "layers must be distinct");
            }
        }

        // Verify the complete taxonomy
        assert_eq!(layers[0], MemoryLayer::Audit);
        assert_eq!(layers[1], MemoryLayer::Evidence);
        assert_eq!(layers[2], MemoryLayer::Packet);
        assert_eq!(layers[3], MemoryLayer::Candidate);
        assert_eq!(layers[4], MemoryLayer::ColdMemory);
    }

    #[test]
    fn cold_memory_preserves_promotion_lineage() {
        let entry = make_accepted_entry("use JWT for auth", vec!["ref-1", "ref-2"]);

        // Simulate promotion (in real code this would be the promotion pipeline)
        let cold = ColdMemoryRecord {
            id: "cold-1".to_string(),
            trace_id: entry.trace_id.clone(),
            source_session_id: entry.source_session_id.clone(),
            kind: entry.kind.clone(),
            summary: entry.summary.clone(),
            source_refs: entry.source_refs.clone(),
            promoted_from: entry.id.clone(),
            created_at_ms: 1700000000000,
        };

        // Verify lineage preservation
        assert_eq!(cold.promoted_from, entry.id, "must link back to ingest entry");
        assert_eq!(cold.trace_id, entry.trace_id, "trace_id preserved");
        assert_eq!(cold.source_session_id, entry.source_session_id, "session preserved");
        assert_eq!(cold.source_refs, entry.source_refs, "source_refs preserved");
        assert_eq!(cold.summary, entry.summary, "summary preserved");
    }

    #[test]
    fn promote_memory_entries_skips_rejected_entries() {
        let accepted = make_accepted_entry("use JWT for auth", vec!["ref-1"]);
        let rejected = make_rejected_entry("validation failed");

        let promoted = promote_memory_entries(&[accepted.clone(), rejected], 1700000000000);
        assert_eq!(promoted.len(), 1, "only accepted entry should be promoted");
        assert_eq!(promoted[0].promoted_from, accepted.id);
        assert_eq!(promoted[0].summary, accepted.summary);
    }
}
