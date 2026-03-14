//! Candidate Consumers — normalize Hot Brain outputs into actionable forms.
//!
//! Pure functions that consume `SchedulerHint[]` and `MemoryCandidate[]`
//! from `CandidateSet`, applying dedup, classification, validation, and bounding.
//!
//! Design sources: specs/17-18 (candidate consumers), specs/21-22 (scheduler normalized outputs).

use serde::{Deserialize, Serialize};

use super::guard::RiskLevel;
use super::hot_brain::{MemoryCandidate, MemoryCandidateKind, SchedulerHint, SchedulerHintKind};

// ── Configuration ────────────────────────────────────────────────────

/// Limits for consumer normalization, independent of HotBrainConfig.
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Maximum scheduler outputs after normalization.
    pub max_scheduler_outputs: usize,
    /// Maximum memory ingest entries after normalization.
    pub max_memory_entries: usize,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            max_scheduler_outputs: 5,
            max_memory_entries: 5,
        }
    }
}

// ── Scheduler Disposition ────────────────────────────────────────────

/// Classification of what should happen with a scheduler hint (spec 21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerDisposition {
    /// Weak, duplicate, or stale — drop silently.
    Ignore,
    /// Worth preserving for audit, but no coordination action needed.
    RecordOnly,
    /// Blocker exists, cross-session plausible — track for future coordination.
    PendingCoordination,
    /// High urgency with broad impact — surface to human operator.
    NeedsHumanReview,
    /// Clear next step with bounded impact — propose a follow-up action.
    ProposeFollowup,
}

// ── Scheduler Normalized Output ──────────────────────────────────────

/// A normalized, classified scheduler output ready for downstream consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerNormalizedOutput {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub disposition: SchedulerDisposition,
    pub reason: String,
    pub urgency_level: RiskLevel,
}

// ── Memory Ingest Entry ──────────────────────────────────────────────

/// A validated, normalized memory candidate ready for the promotion ladder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIngestEntry {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub kind: MemoryCandidateKind,
    pub summary: String,
    pub source_refs: Vec<String>,
    /// Whether this entry passed validation and dedup.
    pub accepted: bool,
    /// Reason for rejection, if any.
    pub rejection_reason: Option<String>,
}

// ── Urgency Helpers ──────────────────────────────────────────────────

/// Map RiskLevel to a numeric rank for comparison (higher = more urgent).
fn urgency_rank(level: &RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    }
}

/// Returns true if urgency is High or Critical.
fn is_high_urgency(level: &RiskLevel) -> bool {
    matches!(level, RiskLevel::High | RiskLevel::Critical)
}

/// Returns true if target list has more than one distinct session.
fn is_multi_target(targets: &[String]) -> bool {
    targets.len() > 1
}

// ── Disposition Classification ───────────────────────────────────────

/// Deterministic classification of a scheduler hint into a disposition.
///
/// Rules (from spec 21):
/// ```text
/// ResolveBlocker + Critical/High + multi-target  -> NeedsHumanReview
/// ResolveBlocker + Critical/High + single-target -> ProposeFollowup
/// ResolveBlocker + Medium/Low                    -> PendingCoordination
///
/// EscalateUrgency + Critical/High + multi-target -> NeedsHumanReview
/// EscalateUrgency + Critical/High + single-target -> ProposeFollowup
/// EscalateUrgency + Medium/Low                   -> RecordOnly
///
/// CoordinateNextSteps + Critical/High            -> ProposeFollowup
/// CoordinateNextSteps + Medium                   -> RecordOnly
/// CoordinateNextSteps + Low                      -> Ignore
/// ```
fn classify_disposition(
    kind: &SchedulerHintKind,
    urgency: &RiskLevel,
    targets: &[String],
) -> SchedulerDisposition {
    match kind {
        SchedulerHintKind::ResolveBlocker => {
            if is_high_urgency(urgency) {
                if is_multi_target(targets) {
                    SchedulerDisposition::NeedsHumanReview
                } else {
                    SchedulerDisposition::ProposeFollowup
                }
            } else {
                SchedulerDisposition::PendingCoordination
            }
        }
        SchedulerHintKind::EscalateUrgency => {
            if is_high_urgency(urgency) {
                if is_multi_target(targets) {
                    SchedulerDisposition::NeedsHumanReview
                } else {
                    SchedulerDisposition::ProposeFollowup
                }
            } else {
                SchedulerDisposition::RecordOnly
            }
        }
        SchedulerHintKind::CoordinateNextSteps => {
            if is_high_urgency(urgency) {
                SchedulerDisposition::ProposeFollowup
            } else if matches!(urgency, RiskLevel::Medium) {
                SchedulerDisposition::RecordOnly
            } else {
                SchedulerDisposition::Ignore
            }
        }
    }
}

// ── Normalize Scheduler Hints ────────────────────────────────────────

/// Normalize scheduler hints: dedup, classify, bound.
///
/// Dedup groups by `(source_session_id, hint_kind)`. Within each group,
/// the highest urgency wins and reasons are concatenated.
///
/// Pure function: same inputs always produce the same outputs.
pub fn normalize_scheduler_hints(
    hints: &[SchedulerHint],
    config: &ConsumerConfig,
    trace_id: &str,
) -> Vec<SchedulerNormalizedOutput> {
    // Group by (source_session_id, hint_kind) for dedup
    let mut groups: Vec<(String, SchedulerHintKind, Vec<&SchedulerHint>)> = Vec::new();

    for hint in hints {
        let found = groups.iter_mut().find(|(sid, kind, _)| {
            sid == &hint.source_session_id && kind == &hint.hint_kind
        });
        if let Some((_, _, members)) = found {
            members.push(hint);
        } else {
            groups.push((
                hint.source_session_id.clone(),
                hint.hint_kind.clone(),
                vec![hint],
            ));
        }
    }

    let mut outputs = Vec::new();

    for (_session_id, _kind, members) in &groups {
        // Highest urgency wins
        let best = members
            .iter()
            .max_by_key(|h| urgency_rank(&h.urgency_level))
            .expect("group is non-empty");

        // Concatenate reasons from all members
        let combined_reason: String = members
            .iter()
            .map(|h| h.reason.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        // Merge all target_session_ids (deduplicated)
        let mut all_targets = Vec::new();
        for m in members {
            for t in &m.target_session_ids {
                if !all_targets.contains(t) {
                    all_targets.push(t.clone());
                }
            }
        }

        let disposition = classify_disposition(
            &best.hint_kind,
            &best.urgency_level,
            &all_targets,
        );

        outputs.push(SchedulerNormalizedOutput {
            id: best.id.clone(),
            trace_id: trace_id.to_string(),
            source_session_id: best.source_session_id.clone(),
            target_session_ids: all_targets,
            disposition,
            reason: combined_reason,
            urgency_level: best.urgency_level.clone(),
        });
    }

    // Bound output count
    outputs.truncate(config.max_scheduler_outputs);

    outputs
}

// ── Normalize Memory Candidates ──────────────────────────────────────

/// Normalize memory candidates: validate, dedup, bound.
///
/// Validation: non-empty source_session_id, source_refs, summary.
/// Failures produce entries with `accepted: false`.
///
/// Dedup groups by `(source_session_id, kind, summary)`. Keeps entry
/// with the most source_refs.
///
/// Pure function: same inputs always produce the same outputs.
pub fn normalize_memory_candidates(
    candidates: &[MemoryCandidate],
    config: &ConsumerConfig,
    trace_id: &str,
) -> Vec<MemoryIngestEntry> {
    let mut entries = Vec::new();

    // First pass: validate each candidate
    for c in candidates {
        if c.source_session_id.is_empty() {
            entries.push(MemoryIngestEntry {
                id: c.id.clone(),
                trace_id: trace_id.to_string(),
                source_session_id: c.source_session_id.clone(),
                kind: c.kind.clone(),
                summary: c.summary.clone(),
                source_refs: c.source_refs.clone(),
                accepted: false,
                rejection_reason: Some("empty source_session_id".to_string()),
            });
            continue;
        }
        if c.source_refs.is_empty() {
            entries.push(MemoryIngestEntry {
                id: c.id.clone(),
                trace_id: trace_id.to_string(),
                source_session_id: c.source_session_id.clone(),
                kind: c.kind.clone(),
                summary: c.summary.clone(),
                source_refs: c.source_refs.clone(),
                accepted: false,
                rejection_reason: Some("empty source_refs".to_string()),
            });
            continue;
        }
        if c.summary.trim().is_empty() {
            entries.push(MemoryIngestEntry {
                id: c.id.clone(),
                trace_id: trace_id.to_string(),
                source_session_id: c.source_session_id.clone(),
                kind: c.kind.clone(),
                summary: c.summary.clone(),
                source_refs: c.source_refs.clone(),
                accepted: false,
                rejection_reason: Some("empty summary".to_string()),
            });
            continue;
        }

        // Validated — tentatively accepted
        entries.push(MemoryIngestEntry {
            id: c.id.clone(),
            trace_id: trace_id.to_string(),
            source_session_id: c.source_session_id.clone(),
            kind: c.kind.clone(),
            summary: c.summary.clone(),
            source_refs: c.source_refs.clone(),
            accepted: true,
            rejection_reason: None,
        });
    }

    // Second pass: dedup accepted entries by (source_session_id, kind, summary).
    // Keep the one with the most source_refs; mark duplicates as rejected.
    let mut seen: Vec<(String, MemoryCandidateKind, String, usize)> = Vec::new(); // (sid, kind, summary, index)

    for i in 0..entries.len() {
        if !entries[i].accepted {
            continue;
        }
        let key = (
            entries[i].source_session_id.clone(),
            entries[i].kind.clone(),
            entries[i].summary.clone(),
        );
        if let Some(existing) = seen.iter().find(|(s, k, sum, _)| {
            s == &key.0 && k == &key.1 && sum == &key.2
        }) {
            let existing_idx = existing.3;
            // Compare source_refs count — keep the one with more
            if entries[i].source_refs.len() > entries[existing_idx].source_refs.len() {
                // New entry wins — mark old as duplicate
                entries[existing_idx].accepted = false;
                entries[existing_idx].rejection_reason =
                    Some("duplicate (fewer source_refs)".to_string());
                // Update seen to point to new winner
                let pos = seen
                    .iter()
                    .position(|(s, k, sum, _)| s == &key.0 && k == &key.1 && sum == &key.2)
                    .unwrap();
                seen[pos].3 = i;
            } else {
                // Existing wins — mark new as duplicate
                entries[i].accepted = false;
                entries[i].rejection_reason = Some("duplicate (fewer source_refs)".to_string());
            }
        } else {
            seen.push((key.0, key.1, key.2, i));
        }
    }

    // Bound output count (rejected entries still included for audit trail)
    entries.truncate(config.max_memory_entries);

    entries
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::guard;

    fn make_hint(
        session_id: &str,
        kind: SchedulerHintKind,
        urgency: RiskLevel,
        targets: Vec<&str>,
        reason: &str,
    ) -> SchedulerHint {
        SchedulerHint {
            id: guard::short_id(),
            trace_id: "trace-test".to_string(),
            source_session_id: session_id.to_string(),
            target_session_ids: targets.into_iter().map(String::from).collect(),
            hint_kind: kind,
            reason: reason.to_string(),
            urgency_level: urgency,
        }
    }

    fn make_candidate(
        session_id: &str,
        kind: MemoryCandidateKind,
        summary: &str,
        refs: Vec<&str>,
    ) -> MemoryCandidate {
        MemoryCandidate {
            id: guard::short_id(),
            trace_id: "trace-test".to_string(),
            source_session_id: session_id.to_string(),
            kind,
            summary: summary.to_string(),
            source_refs: refs.into_iter().map(String::from).collect(),
        }
    }

    // ── Spec-required tests (7) ──────────────────────────────────────

    #[test]
    fn scheduler_hints_normalize_deterministically() {
        let config = ConsumerConfig::default();
        let hints = vec![
            make_hint("s1", SchedulerHintKind::ResolveBlocker, RiskLevel::High, vec!["s1"], "blocker A"),
            make_hint("s1", SchedulerHintKind::EscalateUrgency, RiskLevel::Critical, vec!["s1"], "urgent"),
        ];

        let r1 = normalize_scheduler_hints(&hints, &config, "t1");
        let r2 = normalize_scheduler_hints(&hints, &config, "t1");

        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.disposition, b.disposition);
            assert_eq!(a.reason, b.reason);
            assert_eq!(a.source_session_id, b.source_session_id);
        }
    }

    #[test]
    fn memory_candidates_normalize_deterministically() {
        let config = ConsumerConfig::default();
        let candidates = vec![
            make_candidate("s1", MemoryCandidateKind::Decision, "use JWT", vec!["ref-1"]),
            make_candidate("s2", MemoryCandidateKind::Finding, "API deprecated", vec!["ref-2"]),
        ];

        let r1 = normalize_memory_candidates(&candidates, &config, "t1");
        let r2 = normalize_memory_candidates(&candidates, &config, "t1");

        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.accepted, b.accepted);
            assert_eq!(a.summary, b.summary);
            assert_eq!(a.kind, b.kind);
        }
    }

    #[test]
    fn source_refs_preserved_through_normalization() {
        let config = ConsumerConfig::default();
        let candidates = vec![
            make_candidate("s1", MemoryCandidateKind::Decision, "decision A", vec!["ref-1", "ref-2"]),
        ];

        let result = normalize_memory_candidates(&candidates, &config, "t1");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_refs, vec!["ref-1", "ref-2"]);
    }

    #[test]
    fn duplicate_candidates_collapse_safely() {
        let config = ConsumerConfig::default();
        // Two candidates with same (session, kind, summary) — the one with more refs wins
        let candidates = vec![
            make_candidate("s1", MemoryCandidateKind::Decision, "use JWT", vec!["ref-1"]),
            make_candidate("s1", MemoryCandidateKind::Decision, "use JWT", vec!["ref-1", "ref-2", "ref-3"]),
        ];

        let result = normalize_memory_candidates(&candidates, &config, "t1");

        let accepted: Vec<_> = result.iter().filter(|e| e.accepted).collect();
        assert_eq!(accepted.len(), 1, "only one should survive dedup");
        assert_eq!(accepted[0].source_refs.len(), 3, "winner should have most refs");
    }

    #[test]
    fn no_world_mutation_side_effects() {
        let config = ConsumerConfig::default();
        let hints = vec![
            make_hint("s1", SchedulerHintKind::ResolveBlocker, RiskLevel::High, vec!["s1"], "blocker"),
        ];
        let candidates = vec![
            make_candidate("s1", MemoryCandidateKind::Finding, "finding", vec!["ref-1"]),
        ];

        // Snapshot inputs
        let hints_json = serde_json::to_string(&hints).unwrap();
        let candidates_json = serde_json::to_string(&candidates).unwrap();

        // Run normalization
        let _sched = normalize_scheduler_hints(&hints, &config, "t1");
        let _mem = normalize_memory_candidates(&candidates, &config, "t1");

        // Inputs unchanged
        assert_eq!(serde_json::to_string(&hints).unwrap(), hints_json);
        assert_eq!(serde_json::to_string(&candidates).unwrap(), candidates_json);
    }

    #[test]
    fn deduplication_of_similar_blocker_hints() {
        let config = ConsumerConfig::default();
        // Two ResolveBlocker hints from the same session — should collapse to one
        let hints = vec![
            make_hint("s1", SchedulerHintKind::ResolveBlocker, RiskLevel::Low, vec!["s1"], "timeout issue"),
            make_hint("s1", SchedulerHintKind::ResolveBlocker, RiskLevel::High, vec!["s1"], "auth issue"),
        ];

        let result = normalize_scheduler_hints(&hints, &config, "t1");
        assert_eq!(result.len(), 1, "same (session, kind) should collapse");
        // Highest urgency wins
        assert!(matches!(result[0].urgency_level, RiskLevel::High));
        // Both reasons concatenated
        assert!(result[0].reason.contains("timeout issue"));
        assert!(result[0].reason.contains("auth issue"));
    }

    #[test]
    fn deterministic_classification_hint_to_disposition() {
        // ResolveBlocker + High + multi-target → NeedsHumanReview
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::ResolveBlocker,
                &RiskLevel::High,
                &["s1".into(), "s2".into()],
            ),
            SchedulerDisposition::NeedsHumanReview,
        );

        // ResolveBlocker + High + single-target → ProposeFollowup
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::ResolveBlocker,
                &RiskLevel::High,
                &["s1".into()],
            ),
            SchedulerDisposition::ProposeFollowup,
        );

        // ResolveBlocker + Low → PendingCoordination
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::ResolveBlocker,
                &RiskLevel::Low,
                &["s1".into()],
            ),
            SchedulerDisposition::PendingCoordination,
        );

        // EscalateUrgency + Critical + multi → NeedsHumanReview
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::EscalateUrgency,
                &RiskLevel::Critical,
                &["s1".into(), "s2".into()],
            ),
            SchedulerDisposition::NeedsHumanReview,
        );

        // EscalateUrgency + Medium → RecordOnly
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::EscalateUrgency,
                &RiskLevel::Medium,
                &["s1".into()],
            ),
            SchedulerDisposition::RecordOnly,
        );

        // CoordinateNextSteps + High → ProposeFollowup
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::CoordinateNextSteps,
                &RiskLevel::High,
                &["s1".into()],
            ),
            SchedulerDisposition::ProposeFollowup,
        );

        // CoordinateNextSteps + Medium → RecordOnly
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::CoordinateNextSteps,
                &RiskLevel::Medium,
                &["s1".into()],
            ),
            SchedulerDisposition::RecordOnly,
        );

        // CoordinateNextSteps + Low → Ignore
        assert_eq!(
            classify_disposition(
                &SchedulerHintKind::CoordinateNextSteps,
                &RiskLevel::Low,
                &["s1".into()],
            ),
            SchedulerDisposition::Ignore,
        );
    }

    // ── Additional coverage (4) ──────────────────────────────────────

    #[test]
    fn memory_candidate_rejected_when_empty_source_refs() {
        let config = ConsumerConfig::default();
        let candidates = vec![
            make_candidate("s1", MemoryCandidateKind::Decision, "some decision", vec![]),
        ];

        let result = normalize_memory_candidates(&candidates, &config, "t1");
        assert_eq!(result.len(), 1);
        assert!(!result[0].accepted);
        assert_eq!(result[0].rejection_reason.as_deref(), Some("empty source_refs"));
    }

    #[test]
    fn memory_candidate_rejected_when_empty_source_session_id() {
        let config = ConsumerConfig::default();
        let candidates = vec![
            MemoryCandidate {
                id: guard::short_id(),
                trace_id: "t1".to_string(),
                source_session_id: String::new(),
                kind: MemoryCandidateKind::Finding,
                summary: "some finding".to_string(),
                source_refs: vec!["ref-1".to_string()],
            },
        ];

        let result = normalize_memory_candidates(&candidates, &config, "t1");
        assert_eq!(result.len(), 1);
        assert!(!result[0].accepted);
        assert_eq!(result[0].rejection_reason.as_deref(), Some("empty source_session_id"));
    }

    #[test]
    fn output_count_bounded_by_config() {
        let config = ConsumerConfig {
            max_scheduler_outputs: 2,
            max_memory_entries: 2,
        };

        // Generate 5 distinct hints (different sessions → no dedup)
        let hints: Vec<_> = (0..5)
            .map(|i| {
                make_hint(
                    &format!("s{}", i),
                    SchedulerHintKind::ResolveBlocker,
                    RiskLevel::High,
                    vec!["target"],
                    &format!("reason {}", i),
                )
            })
            .collect();

        // Generate 5 distinct candidates (different sessions → no dedup)
        let candidates: Vec<_> = (0..5)
            .map(|i| {
                make_candidate(
                    &format!("s{}", i),
                    MemoryCandidateKind::Decision,
                    &format!("decision {}", i),
                    vec!["ref-1"],
                )
            })
            .collect();

        let sched = normalize_scheduler_hints(&hints, &config, "t1");
        let mem = normalize_memory_candidates(&candidates, &config, "t1");

        assert!(sched.len() <= 2, "scheduler outputs bounded to 2, got {}", sched.len());
        assert!(mem.len() <= 2, "memory entries bounded to 2, got {}", mem.len());
    }

    #[test]
    fn empty_inputs_produce_empty_outputs() {
        let config = ConsumerConfig::default();

        let sched = normalize_scheduler_hints(&[], &config, "t1");
        let mem = normalize_memory_candidates(&[], &config, "t1");

        assert!(sched.is_empty());
        assert!(mem.is_empty());
    }
}
