//! Hot Brain V1 — bounded, packet-driven coordination analyzer.
//!
//! Reads bounded world slices, performs one deterministic analysis pass,
//! emits candidate outputs. Never mutates world state directly.
//!
//! Design source: specs/12-hot-brain-runtime.md, specs/13-hot-brain-execution-brief.md

use serde::{Deserialize, Serialize};

use super::guard::{self, FeedbackPacket, GuardedCommit, RiskLevel};

// ── Configuration ────────────────────────────────────────────────────

/// Limits that constrain Hot Brain's input consumption and output emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotBrainConfig {
    /// Maximum recent feedback packets to include in a coordination slice.
    #[serde(default = "default_max_packets")]
    pub max_recent_packets: usize,
    /// Maximum affected targets to track in a coordination slice.
    #[serde(default = "default_max_targets")]
    pub max_affected_targets: usize,
    /// Maximum scheduler hints per candidate set.
    #[serde(default = "default_max_hints")]
    pub max_scheduler_hints: usize,
    /// Maximum memory candidates per candidate set.
    #[serde(default = "default_max_memories")]
    pub max_memory_candidates: usize,
}

fn default_max_packets() -> usize {
    3
}
fn default_max_targets() -> usize {
    4
}
fn default_max_hints() -> usize {
    3
}
fn default_max_memories() -> usize {
    3
}

impl Default for HotBrainConfig {
    fn default() -> Self {
        Self {
            max_recent_packets: default_max_packets(),
            max_affected_targets: default_max_targets(),
            max_scheduler_hints: default_max_hints(),
            max_memory_candidates: default_max_memories(),
        }
    }
}

// ── World Slices ─────────────────────────────────────────────────────

/// A bounded, task-relevant subset of world state.
///
/// Hot Brain only reads through slices — never the full world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldSlice {
    /// One session's recent local state (self-follow-up reasoning).
    SessionTurn(SessionTurnSlice),
    /// One session plus direct confirmed neighbors (cross-session relevance).
    Neighborhood(NeighborhoodSlice),
    /// Current coordination frontier (scheduler hints, memory selection).
    Coordination(CoordinationSlice),
}

/// Recent local state for a single session.
///
/// V1 material limits (not enforced yet — this slice is not the primary analyzer input):
/// - max recent packets: same as `HotBrainConfig::max_recent_packets`
/// - max recent commits: 10
/// - max progress refs: 50
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurnSlice {
    pub session_key: String,
    pub session_id: Option<String>,
    pub current_status: String,
    pub recent_packets: Vec<FeedbackPacket>,
    pub recent_commits: Vec<GuardedCommit>,
    /// Bounded references to recent progress entries (not full content).
    pub recent_progress_refs: Vec<String>,
}

/// One session plus nearby related state (bounded neighborhood).
///
/// V1 material limits (not enforced yet — this slice is not the primary analyzer input):
/// - max direct neighbors: 4
/// - relation depth: 1 (direct confirmed only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborhoodSlice {
    pub focal_session: SessionTurnSlice,
    pub neighbor_sessions: Vec<SessionTurnSlice>,
    pub shared_blockers: Vec<String>,
    /// IDs of confirmed relations that justified neighbor inclusion.
    pub confirmed_relation_ids: Vec<String>,
}

/// The coordination frontier — what Hot Brain V1 actually analyzes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationSlice {
    /// Recent feedback packets (bounded by config.max_recent_packets).
    pub recent_packets: Vec<FeedbackPacket>,
    /// Aggregated pending blockers across recent packets.
    pub pending_blockers: Vec<String>,
    /// Aggregated affected targets across recent packets (bounded).
    pub affected_targets: Vec<String>,
}

// ── Candidate Outputs ────────────────────────────────────────────────

/// Non-authoritative recommendation about what coordination action may be useful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerHint {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub hint_kind: SchedulerHintKind,
    pub reason: String,
    pub urgency_level: RiskLevel,
}

/// Categories of scheduler hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerHintKind {
    /// A session has unresolved blockers needing attention.
    ResolveBlocker,
    /// A session's urgency warrants prioritization.
    EscalateUrgency,
    /// A session has next steps that may benefit from coordination.
    CoordinateNextSteps,
}

/// Candidate statement that a turn produced something worth promoting
/// into longer-lived memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub kind: MemoryCandidateKind,
    pub summary: String,
    pub source_refs: Vec<String>,
}

/// Categories of memory-worthy outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryCandidateKind {
    /// A stable decision was made.
    Decision,
    /// A reusable finding was produced.
    Finding,
    /// A repeated blocker pattern was detected.
    RepeatedBlocker,
}

/// Bounded output of one Hot Brain analysis pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSet {
    pub trace_id: String,
    pub created_at_ms: u64,
    pub scheduler_hints: Vec<SchedulerHint>,
    pub memory_candidates: Vec<MemoryCandidate>,
}

// ── Slice Builder ────────────────────────────────────────────────────

/// Build a bounded CoordinationSlice from recent feedback packets.
///
/// Sorts packets by `created_at_ms` internally, then keeps the most recent
/// `config.max_recent_packets`. Does not rely on caller ordering.
///
/// Enforces material limits from config. This is the only input path
/// for Hot Brain V1 — no full graph walks, no raw transcripts.
pub fn build_coordination_slice(
    packets: &[FeedbackPacket],
    config: &HotBrainConfig,
) -> CoordinationSlice {
    // Sort by created_at_ms ascending, then keep the most recent N
    let mut sorted: Vec<FeedbackPacket> = packets.to_vec();
    sorted.sort_by_key(|p| p.created_at_ms);
    let bounded_packets: Vec<FeedbackPacket> = sorted
        .into_iter()
        .rev()
        .take(config.max_recent_packets)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Aggregate blockers across all bounded packets (deduplicated)
    let mut blockers = Vec::new();
    for p in &bounded_packets {
        for b in &p.blockers {
            if !blockers.contains(b) {
                blockers.push(b.clone());
            }
        }
    }

    // Aggregate affected targets (deduplicated, bounded)
    let mut targets = Vec::new();
    for p in &bounded_packets {
        for t in &p.affected_targets {
            if !targets.contains(t) && targets.len() < config.max_affected_targets {
                targets.push(t.clone());
            }
        }
    }

    CoordinationSlice {
        recent_packets: bounded_packets,
        pending_blockers: blockers,
        affected_targets: targets,
    }
}

// ── Deterministic Analysis Pass ──────────────────────────────────────

/// Run one deterministic Hot Brain analysis pass.
///
/// Flow: build slice → analyze → emit candidate set → stop.
/// No recursive self-triggering. No world mutation.
///
/// This is a pure function: same inputs always produce same outputs
/// (modulo generated IDs).
pub fn analyze(
    slice: &CoordinationSlice,
    config: &HotBrainConfig,
    trace_id: &str,
    ts_ms: u64,
) -> CandidateSet {
    let mut hints = Vec::new();
    let mut memories = Vec::new();

    for packet in &slice.recent_packets {
        // ── Scheduler hints ──────────────────────────────────────

        // Blockers → ResolveBlocker hint
        if !packet.blockers.is_empty() && hints.len() < config.max_scheduler_hints {
            hints.push(SchedulerHint {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: packet.source_session_id.clone(),
                target_session_ids: vec![packet.source_session_id.clone()],
                hint_kind: SchedulerHintKind::ResolveBlocker,
                reason: format!(
                    "session has {} unresolved blocker(s): {}",
                    packet.blockers.len(),
                    packet.blockers.join("; ")
                ),
                urgency_level: packet.urgency_level.clone(),
            });
        }

        // High/Critical urgency → EscalateUrgency hint
        if matches!(packet.urgency_level, RiskLevel::High | RiskLevel::Critical)
            && hints.len() < config.max_scheduler_hints
        {
            hints.push(SchedulerHint {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: packet.source_session_id.clone(),
                target_session_ids: vec![packet.source_session_id.clone()],
                hint_kind: SchedulerHintKind::EscalateUrgency,
                reason: format!(
                    "session urgency is {:?}, may need prioritization",
                    packet.urgency_level
                ),
                urgency_level: packet.urgency_level.clone(),
            });
        }

        // Non-empty next_steps → CoordinateNextSteps hint
        if !packet.next_steps.is_empty() && hints.len() < config.max_scheduler_hints {
            hints.push(SchedulerHint {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: packet.source_session_id.clone(),
                target_session_ids: vec![packet.source_session_id.clone()],
                hint_kind: SchedulerHintKind::CoordinateNextSteps,
                reason: format!(
                    "{} pending next step(s)",
                    packet.next_steps.len()
                ),
                urgency_level: packet.urgency_level.clone(),
            });
        }

        // ── Memory candidates ────────────────────────────────────

        // Decisions → Decision memory candidate
        if !packet.decisions.is_empty() && memories.len() < config.max_memory_candidates {
            memories.push(MemoryCandidate {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: packet.source_session_id.clone(),
                kind: MemoryCandidateKind::Decision,
                summary: packet.decisions.join("; "),
                source_refs: packet.source_refs.clone(),
            });
        }

        // Findings → Finding memory candidate
        if !packet.findings.is_empty() && memories.len() < config.max_memory_candidates {
            memories.push(MemoryCandidate {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: packet.source_session_id.clone(),
                kind: MemoryCandidateKind::Finding,
                summary: packet.findings.join("; "),
                source_refs: packet.source_refs.clone(),
            });
        }
    }

    // Repeated blocker detection: if the same blocker appears across
    // multiple packets, it is a pattern worth remembering.
    // Track which packets contributed each blocker for provenance.
    let mut blocker_sources: std::collections::HashMap<
        &str,
        Vec<(&str, &str)>, // (source_session_id, packet_id)
    > = std::collections::HashMap::new();
    for packet in &slice.recent_packets {
        for b in &packet.blockers {
            blocker_sources
                .entry(b.as_str())
                .or_default()
                .push((packet.source_session_id.as_str(), packet.packet_id.as_str()));
        }
    }
    for (blocker, sources) in &blocker_sources {
        if sources.len() > 1 && memories.len() < config.max_memory_candidates {
            // Use the first contributing session as source_session_id;
            // include all contributing packet IDs as source_refs (bounded).
            let first_session = sources[0].0.to_string();
            let packet_refs: Vec<String> = sources
                .iter()
                .take(config.max_recent_packets) // bound provenance list
                .map(|(_, pid)| format!("packet:{}", pid))
                .collect();
            memories.push(MemoryCandidate {
                id: guard::short_id(),
                trace_id: trace_id.to_string(),
                source_session_id: first_session,
                kind: MemoryCandidateKind::RepeatedBlocker,
                summary: format!(
                    "blocker '{}' appeared in {} packets — may be systemic",
                    blocker,
                    sources.len()
                ),
                source_refs: packet_refs,
            });
        }
    }

    // Enforce output caps (truncate if analysis produced too many)
    hints.truncate(config.max_scheduler_hints);
    memories.truncate(config.max_memory_candidates);

    CandidateSet {
        trace_id: trace_id.to_string(),
        created_at_ms: ts_ms,
        scheduler_hints: hints,
        memory_candidates: memories,
    }
}

// ── Session Turn & Neighborhood Slice Builders ──────────────────────

/// Build a SessionTurnSlice for a specific session, filtering and bounding
/// packets and commits to that session's recent activity.
pub fn build_session_turn_slice(
    session_key: &str,
    packets: &[FeedbackPacket],
    commits: &[GuardedCommit],
    config: &HotBrainConfig,
) -> SessionTurnSlice {
    // 1. Filter packets by source_session_id == session_key
    let mut session_packets: Vec<FeedbackPacket> = packets
        .iter()
        .filter(|p| p.source_session_id == session_key)
        .cloned()
        .collect();

    // 2. Sort by created_at_ms ascending
    session_packets.sort_by_key(|p| p.created_at_ms);

    // 3. Take last config.max_recent_packets
    let bounded_packets: Vec<FeedbackPacket> = if session_packets.len() > config.max_recent_packets
    {
        session_packets
            .split_off(session_packets.len() - config.max_recent_packets)
    } else {
        session_packets
    };

    // 4. Filter commits by session_key, sort by created_at_ms, take last 10
    let mut session_commits: Vec<GuardedCommit> = commits
        .iter()
        .filter(|c| c.session_key == session_key)
        .cloned()
        .collect();
    session_commits.sort_by_key(|c| c.created_at_ms);
    let bounded_commits: Vec<GuardedCommit> = if session_commits.len() > 10 {
        session_commits.split_off(session_commits.len() - 10)
    } else {
        session_commits
    };

    // 5. Extract progress refs from packet source_refs (take last 50)
    let mut progress_refs: Vec<String> = bounded_packets
        .iter()
        .flat_map(|p| p.source_refs.iter().cloned())
        .collect();
    if progress_refs.len() > 50 {
        progress_refs = progress_refs.split_off(progress_refs.len() - 50);
    }

    // 6. current_status from most recent packet's `now` field (or "unknown")
    let current_status = bounded_packets
        .last()
        .and_then(|p| p.now.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // 7. session_id from matching packets' source_session_id
    let session_id = bounded_packets
        .first()
        .map(|p| p.source_session_id.clone());

    SessionTurnSlice {
        session_key: session_key.to_string(),
        session_id,
        current_status,
        recent_packets: bounded_packets,
        recent_commits: bounded_commits,
        recent_progress_refs: progress_refs,
    }
}

/// Build a NeighborhoodSlice centered on a focal session, including
/// direct neighbors from confirmed relationships.
pub fn build_neighborhood_slice(
    focal_session: &str,
    all_packets: &[FeedbackPacket],
    all_commits: &[GuardedCommit],
    relationships: &[(String, String)], // (session_a, session_b) pairs
    config: &HotBrainConfig,
) -> NeighborhoodSlice {
    // 1. Build focal SessionTurnSlice
    let focal = build_session_turn_slice(focal_session, all_packets, all_commits, config);

    // 2. Find neighbor session_keys from relationships where focal is one end
    let mut neighbor_keys: Vec<String> = Vec::new();
    let mut used_relations: Vec<String> = Vec::new();
    for (a, b) in relationships {
        let neighbor = if a == focal_session {
            Some(b.clone())
        } else if b == focal_session {
            Some(a.clone())
        } else {
            None
        };
        if let Some(key) = neighbor {
            if !neighbor_keys.contains(&key) {
                neighbor_keys.push(key);
                used_relations.push(format!("{}:{}", a, b));
            }
        }
    }

    // 3. Limit to 4 neighbors
    neighbor_keys.truncate(4);
    used_relations.truncate(4);

    // 4. Build SessionTurnSlice for each neighbor
    let neighbor_slices: Vec<SessionTurnSlice> = neighbor_keys
        .iter()
        .map(|key| build_session_turn_slice(key, all_packets, all_commits, config))
        .collect();

    // 5. shared_blockers: blockers in focal AND any neighbor
    let focal_blockers: Vec<String> = focal
        .recent_packets
        .iter()
        .flat_map(|p| p.blockers.iter().cloned())
        .collect();

    let mut shared_blockers: Vec<String> = Vec::new();
    for nb in &neighbor_slices {
        for packet in &nb.recent_packets {
            for blocker in &packet.blockers {
                if focal_blockers.contains(blocker) && !shared_blockers.contains(blocker) {
                    shared_blockers.push(blocker.clone());
                }
            }
        }
    }

    // 6. confirmed_relation_ids from relationship pairs used
    NeighborhoodSlice {
        focal_session: focal,
        neighbor_sessions: neighbor_slices,
        shared_blockers,
        confirmed_relation_ids: used_relations,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::guard::{FeedbackPacket, ResponseLevel, RiskLevel};

    fn make_packet(
        session_id: &str,
        blockers: Vec<&str>,
        decisions: Vec<&str>,
        findings: Vec<&str>,
        next_steps: Vec<&str>,
        urgency: RiskLevel,
    ) -> FeedbackPacket {
        make_packet_at(session_id, blockers, decisions, findings, next_steps, urgency, 1700000000000)
    }

    fn make_packet_at(
        session_id: &str,
        blockers: Vec<&str>,
        decisions: Vec<&str>,
        findings: Vec<&str>,
        next_steps: Vec<&str>,
        urgency: RiskLevel,
        ts_ms: u64,
    ) -> FeedbackPacket {
        FeedbackPacket {
            packet_id: guard::short_id(),
            trace_id: "trace-test".to_string(),
            source_session_id: session_id.to_string(),
            created_at_ms: ts_ms,
            goal: None,
            now: None,
            done_this_turn: vec![],
            blockers: blockers.into_iter().map(String::from).collect(),
            decisions: decisions.into_iter().map(String::from).collect(),
            findings: findings.into_iter().map(String::from).collect(),
            next_steps: next_steps.into_iter().map(String::from).collect(),
            affected_targets: vec!["target-a".to_string()],
            source_refs: vec!["ref-1".to_string()],
            urgency_level: urgency,
            recommended_response_level: ResponseLevel::L2SelfInject,
        }
    }

    #[test]
    fn analyze_emits_scheduler_hints_from_blockers() {
        let config = HotBrainConfig::default();
        let packet = make_packet(
            "sid-1",
            vec!["API rate limit"],
            vec![],
            vec![],
            vec![],
            RiskLevel::Low,
        );
        let slice = build_coordination_slice(&[packet], &config);
        let result = analyze(&slice, &config, "trace-1", 1700000000000);

        assert!(
            !result.scheduler_hints.is_empty(),
            "should emit hint for blocker"
        );
        assert!(matches!(
            result.scheduler_hints[0].hint_kind,
            SchedulerHintKind::ResolveBlocker
        ));
        assert!(result.scheduler_hints[0].reason.contains("API rate limit"));
    }

    #[test]
    fn analyze_emits_memory_candidates_from_decisions_and_findings() {
        let config = HotBrainConfig::default();
        let packet = make_packet(
            "sid-1",
            vec![],
            vec!["use JWT for auth"],
            vec!["legacy API deprecated"],
            vec![],
            RiskLevel::Low,
        );
        let slice = build_coordination_slice(&[packet], &config);
        let result = analyze(&slice, &config, "trace-2", 1700000000000);

        assert!(
            !result.memory_candidates.is_empty(),
            "should emit memory candidates"
        );
        let kinds: Vec<_> = result
            .memory_candidates
            .iter()
            .map(|m| match &m.kind {
                MemoryCandidateKind::Decision => "decision",
                MemoryCandidateKind::Finding => "finding",
                MemoryCandidateKind::RepeatedBlocker => "repeated_blocker",
            })
            .collect();
        assert!(kinds.contains(&"decision"), "should have decision candidate");
        assert!(kinds.contains(&"finding"), "should have finding candidate");
    }

    #[test]
    fn slice_builder_trims_to_material_limit() {
        let config = HotBrainConfig {
            max_recent_packets: 2,
            max_affected_targets: 1,
            ..Default::default()
        };

        // Create 5 packets with distinct timestamps — slice should only keep the 2 most recent
        let packets: Vec<FeedbackPacket> = (0..5)
            .map(|i| {
                make_packet_at(
                    &format!("sid-{}", i),
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    RiskLevel::Low,
                    1700000000000 + (i as u64) * 1000,
                )
            })
            .collect();

        let slice = build_coordination_slice(&packets, &config);
        assert_eq!(
            slice.recent_packets.len(),
            2,
            "slice must trim to max_recent_packets"
        );
        // Should keep the most recent 2 by created_at_ms (sid-3 and sid-4)
        assert_eq!(slice.recent_packets[0].source_session_id, "sid-3");
        assert_eq!(slice.recent_packets[1].source_session_id, "sid-4");

        // affected_targets should be bounded to 1
        assert!(
            slice.affected_targets.len() <= 1,
            "affected_targets must respect max_affected_targets"
        );
    }

    #[test]
    fn output_count_never_exceeds_cap() {
        let config = HotBrainConfig {
            max_recent_packets: 3,
            max_scheduler_hints: 2,
            max_memory_candidates: 1,
            ..Default::default()
        };

        // Create 3 packets, each with blockers + decisions + findings + next_steps
        // This would generate many candidates if uncapped
        let packets: Vec<FeedbackPacket> = (0..3)
            .map(|i| {
                make_packet(
                    &format!("sid-{}", i),
                    vec!["blocker"],
                    vec!["decision"],
                    vec!["finding"],
                    vec!["next step"],
                    RiskLevel::High,
                )
            })
            .collect();

        let slice = build_coordination_slice(&packets, &config);
        let result = analyze(&slice, &config, "trace-cap", 1700000000000);

        assert!(
            result.scheduler_hints.len() <= 2,
            "scheduler_hints must not exceed cap of 2, got {}",
            result.scheduler_hints.len()
        );
        assert!(
            result.memory_candidates.len() <= 1,
            "memory_candidates must not exceed cap of 1, got {}",
            result.memory_candidates.len()
        );
    }

    #[test]
    fn analyze_is_pure_no_mutation() {
        let config = HotBrainConfig::default();
        let packet = make_packet(
            "sid-1",
            vec!["blocker-a"],
            vec!["decision-x"],
            vec![],
            vec!["step-1"],
            RiskLevel::Medium,
        );
        let slice = build_coordination_slice(&[packet.clone()], &config);

        // Snapshot slice state before analysis
        let slice_before = serde_json::to_string(&slice).unwrap();

        // Run analysis
        let _result = analyze(&slice, &config, "trace-pure", 1700000000000);

        // Slice must be unchanged after analysis (read-only)
        let slice_after = serde_json::to_string(&slice).unwrap();
        assert_eq!(
            slice_before, slice_after,
            "analysis must not mutate the input slice"
        );
    }

    #[test]
    fn analyze_escalates_high_urgency() {
        let config = HotBrainConfig::default();
        let packet = make_packet(
            "sid-urgent",
            vec![],
            vec![],
            vec![],
            vec![],
            RiskLevel::Critical,
        );
        let slice = build_coordination_slice(&[packet], &config);
        let result = analyze(&slice, &config, "trace-urg", 1700000000000);

        let escalation_hints: Vec<_> = result
            .scheduler_hints
            .iter()
            .filter(|h| matches!(h.hint_kind, SchedulerHintKind::EscalateUrgency))
            .collect();
        assert!(
            !escalation_hints.is_empty(),
            "Critical urgency should produce escalation hint"
        );
    }

    #[test]
    fn analyze_detects_repeated_blockers_with_provenance() {
        let config = HotBrainConfig::default();
        // Two packets with the same blocker → should produce a RepeatedBlocker memory candidate
        let packets = vec![
            make_packet("sid-1", vec!["auth service down"], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("sid-2", vec!["auth service down"], vec![], vec![], vec![], RiskLevel::Low),
        ];
        let slice = build_coordination_slice(&packets, &config);
        let result = analyze(&slice, &config, "trace-repeat", 1700000000000);

        let repeated: Vec<_> = result
            .memory_candidates
            .iter()
            .filter(|m| matches!(m.kind, MemoryCandidateKind::RepeatedBlocker))
            .collect();
        assert!(
            !repeated.is_empty(),
            "repeated blocker across packets should produce RepeatedBlocker candidate"
        );
        assert!(repeated[0].summary.contains("auth service down"));

        // Provenance: source_session_id must be non-empty (first contributing session)
        assert!(
            !repeated[0].source_session_id.is_empty(),
            "RepeatedBlocker must have traceable source_session_id"
        );
        // source_refs must contain packet IDs from contributing packets
        assert_eq!(
            repeated[0].source_refs.len(),
            2,
            "should reference both contributing packets"
        );
        assert!(
            repeated[0].source_refs.iter().all(|r| r.starts_with("packet:")),
            "source_refs should be packet:<id> format"
        );
    }

    #[test]
    fn slice_builder_sorts_by_timestamp_regardless_of_input_order() {
        let config = HotBrainConfig {
            max_recent_packets: 2,
            ..Default::default()
        };

        // Feed packets in reverse chronological order — slice must still
        // keep the two most recent by created_at_ms, not by position.
        let packets = vec![
            make_packet_at("sid-newest", vec![], vec![], vec![], vec![], RiskLevel::Low, 3000),
            make_packet_at("sid-oldest", vec![], vec![], vec![], vec![], RiskLevel::Low, 1000),
            make_packet_at("sid-middle", vec![], vec![], vec![], vec![], RiskLevel::Low, 2000),
        ];

        let slice = build_coordination_slice(&packets, &config);
        assert_eq!(slice.recent_packets.len(), 2);
        // Should keep middle (2000) and newest (3000), in ascending order
        assert_eq!(slice.recent_packets[0].source_session_id, "sid-middle");
        assert_eq!(slice.recent_packets[1].source_session_id, "sid-newest");
    }

    #[test]
    fn slice_builder_deduplicates_blockers_and_targets() {
        let config = HotBrainConfig::default();

        // Two packets with overlapping blockers and identical targets
        let mut p1 = make_packet("sid-1", vec!["db timeout", "auth down"], vec![], vec![], vec![], RiskLevel::Low);
        p1.affected_targets = vec!["svc-api".into(), "svc-web".into()];

        let mut p2 = make_packet("sid-2", vec!["auth down", "disk full"], vec![], vec![], vec![], RiskLevel::Low);
        p2.affected_targets = vec!["svc-api".into(), "svc-worker".into()];

        let slice = build_coordination_slice(&[p1, p2], &config);

        // "auth down" appears in both packets but should appear once in pending_blockers
        let auth_count = slice.pending_blockers.iter().filter(|b| *b == "auth down").count();
        assert_eq!(auth_count, 1, "duplicate blocker must be deduplicated");

        // All three distinct blockers should be present
        assert_eq!(slice.pending_blockers.len(), 3, "should have 3 unique blockers");
        assert!(slice.pending_blockers.contains(&"db timeout".to_string()));
        assert!(slice.pending_blockers.contains(&"auth down".to_string()));
        assert!(slice.pending_blockers.contains(&"disk full".to_string()));

        // "svc-api" appears in both packets but should appear once in affected_targets
        let api_count = slice.affected_targets.iter().filter(|t| *t == "svc-api").count();
        assert_eq!(api_count, 1, "duplicate target must be deduplicated");

        // All three distinct targets should be present
        assert_eq!(slice.affected_targets.len(), 3, "should have 3 unique targets");
    }

    #[test]
    fn slice_builder_retains_provenance_through_packets() {
        let config = HotBrainConfig::default();

        // Deduped blockers/targets must still be traceable to their source packets
        let mut p1 = make_packet("sid-A", vec!["shared-blocker"], vec![], vec![], vec![], RiskLevel::Low);
        p1.affected_targets = vec!["target-x".into()];

        let mut p2 = make_packet("sid-B", vec!["shared-blocker"], vec![], vec![], vec![], RiskLevel::Low);
        p2.affected_targets = vec!["target-x".into()];

        let slice = build_coordination_slice(&[p1, p2], &config);

        // "shared-blocker" is deduped to 1 entry in pending_blockers
        assert_eq!(slice.pending_blockers.len(), 1);

        // But both source packets are still in recent_packets, so provenance is preserved
        assert_eq!(slice.recent_packets.len(), 2, "both source packets retained");
        let source_sessions: Vec<&str> = slice
            .recent_packets
            .iter()
            .map(|p| p.source_session_id.as_str())
            .collect();
        assert!(source_sessions.contains(&"sid-A"), "packet from sid-A preserved");
        assert!(source_sessions.contains(&"sid-B"), "packet from sid-B preserved");

        // For any deduped blocker, a consumer can find contributing packets by scanning recent_packets
        for blocker in &slice.pending_blockers {
            let contributing: Vec<&str> = slice
                .recent_packets
                .iter()
                .filter(|p| p.blockers.contains(blocker))
                .map(|p| p.source_session_id.as_str())
                .collect();
            assert!(
                !contributing.is_empty(),
                "deduped blocker '{}' must be traceable to at least one packet",
                blocker
            );
        }
    }

    #[test]
    fn empty_packets_produce_empty_candidates() {
        let config = HotBrainConfig::default();
        let slice = build_coordination_slice(&[], &config);
        let result = analyze(&slice, &config, "trace-empty", 1700000000000);

        assert!(result.scheduler_hints.is_empty(), "no packets → no hints");
        assert!(
            result.memory_candidates.is_empty(),
            "no packets → no memory candidates"
        );
    }

    #[test]
    fn trace_id_propagates_through_candidates() {
        let config = HotBrainConfig::default();
        let packet = make_packet(
            "sid-1",
            vec!["blocker"],
            vec!["decision"],
            vec![],
            vec![],
            RiskLevel::Low,
        );
        let slice = build_coordination_slice(&[packet], &config);
        let result = analyze(&slice, &config, "trace-propagate", 1700000000000);

        assert_eq!(result.trace_id, "trace-propagate");
        for hint in &result.scheduler_hints {
            assert_eq!(hint.trace_id, "trace-propagate", "hint trace_id must match");
        }
        for mem in &result.memory_candidates {
            assert_eq!(
                mem.trace_id, "trace-propagate",
                "memory candidate trace_id must match"
            );
        }
    }

    // ── SessionTurnSlice Tests ──────────────────────────────────────

    fn make_commit(session_key: &str, ts_ms: u64) -> GuardedCommit {
        use crate::agent::guard::{Attestation, GuardDecision};
        GuardedCommit {
            commit_id: guard::short_id(),
            trace_id: "trace-test".to_string(),
            session_key: session_key.to_string(),
            proposal_id: guard::short_id(),
            decision: GuardDecision::Approve,
            attestation: Attestation {
                passed: true,
                summary: "ok".to_string(),
                checks: vec![],
            },
            created_at_ms: ts_ms,
        }
    }

    #[test]
    fn session_turn_slice_filters_by_session() {
        let config = HotBrainConfig::default();
        let packets = vec![
            make_packet("sid-A", vec!["b1"], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("sid-B", vec!["b2"], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("sid-A", vec!["b3"], vec![], vec![], vec![], RiskLevel::Low),
        ];
        let commits = vec![
            make_commit("sid-A", 1700000000000),
            make_commit("sid-B", 1700000000000),
        ];

        let slice = build_session_turn_slice("sid-A", &packets, &commits, &config);

        assert_eq!(slice.session_key, "sid-A");
        assert_eq!(slice.recent_packets.len(), 2, "only sid-A packets");
        assert!(
            slice
                .recent_packets
                .iter()
                .all(|p| p.source_session_id == "sid-A"),
            "all packets must belong to sid-A"
        );
        assert_eq!(slice.recent_commits.len(), 1, "only sid-A commits");
        assert!(
            slice
                .recent_commits
                .iter()
                .all(|c| c.session_key == "sid-A"),
            "all commits must belong to sid-A"
        );
    }

    #[test]
    fn session_turn_slice_respects_material_limits() {
        let config = HotBrainConfig {
            max_recent_packets: 2,
            ..Default::default()
        };

        // Create 5 packets for the same session with distinct timestamps
        let packets: Vec<FeedbackPacket> = (0..5)
            .map(|i| {
                make_packet_at(
                    "sid-X",
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    RiskLevel::Low,
                    1700000000000 + (i as u64) * 1000,
                )
            })
            .collect();

        let slice = build_session_turn_slice("sid-X", &packets, &[], &config);

        assert_eq!(
            slice.recent_packets.len(),
            2,
            "must bound to max_recent_packets"
        );
        // Should keep the 2 most recent (ts 3000 and 4000)
        assert_eq!(slice.recent_packets[0].created_at_ms, 1700000003000);
        assert_eq!(slice.recent_packets[1].created_at_ms, 1700000004000);
    }

    #[test]
    fn session_turn_slice_sorts_by_timestamp() {
        let config = HotBrainConfig::default();

        // Create packets in reverse order
        let packets = vec![
            make_packet_at("sid-S", vec![], vec![], vec![], vec![], RiskLevel::Low, 3000),
            make_packet_at("sid-S", vec![], vec![], vec![], vec![], RiskLevel::Low, 1000),
            make_packet_at("sid-S", vec![], vec![], vec![], vec![], RiskLevel::Low, 2000),
        ];

        let slice = build_session_turn_slice("sid-S", &packets, &[], &config);

        assert_eq!(slice.recent_packets.len(), 3);
        assert_eq!(slice.recent_packets[0].created_at_ms, 1000);
        assert_eq!(slice.recent_packets[1].created_at_ms, 2000);
        assert_eq!(slice.recent_packets[2].created_at_ms, 3000);
    }

    #[test]
    fn session_turn_slice_extracts_progress_refs() {
        let config = HotBrainConfig::default();
        // make_packet sets source_refs to ["ref-1"] by default
        let packets = vec![
            make_packet("sid-R", vec![], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("sid-R", vec![], vec![], vec![], vec![], RiskLevel::Low),
        ];

        let slice = build_session_turn_slice("sid-R", &packets, &[], &config);

        assert_eq!(
            slice.recent_progress_refs.len(),
            2,
            "should collect refs from both packets"
        );
        assert!(
            slice
                .recent_progress_refs
                .iter()
                .all(|r| r == "ref-1"),
            "all refs should be 'ref-1' from make_packet default"
        );
    }

    #[test]
    fn session_turn_slice_current_status_from_now_field() {
        let config = HotBrainConfig::default();
        let mut p1 = make_packet_at("sid-N", vec![], vec![], vec![], vec![], RiskLevel::Low, 1000);
        p1.now = Some("working on auth".to_string());
        let mut p2 = make_packet_at("sid-N", vec![], vec![], vec![], vec![], RiskLevel::Low, 2000);
        p2.now = Some("deploying API".to_string());

        let slice = build_session_turn_slice("sid-N", &[p1, p2], &[], &config);

        assert_eq!(
            slice.current_status, "deploying API",
            "should use most recent packet's now field"
        );
    }

    #[test]
    fn session_turn_slice_defaults_to_unknown_status() {
        let config = HotBrainConfig::default();
        // make_packet sets now: None by default
        let packets = vec![make_packet(
            "sid-U",
            vec![],
            vec![],
            vec![],
            vec![],
            RiskLevel::Low,
        )];

        let slice = build_session_turn_slice("sid-U", &packets, &[], &config);

        assert_eq!(
            slice.current_status, "unknown",
            "should default to 'unknown' when now is None"
        );
    }

    // ── NeighborhoodSlice Tests ─────────────────────────────────────

    #[test]
    fn neighborhood_slice_includes_neighbors() {
        let config = HotBrainConfig::default();
        let packets = vec![
            make_packet("focal", vec![], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("nb-1", vec![], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("nb-2", vec![], vec![], vec![], vec![], RiskLevel::Low),
        ];
        let relationships = vec![
            ("focal".to_string(), "nb-1".to_string()),
            ("nb-2".to_string(), "focal".to_string()),
        ];

        let slice = build_neighborhood_slice("focal", &packets, &[], &relationships, &config);

        assert_eq!(slice.focal_session.session_key, "focal");
        assert_eq!(slice.neighbor_sessions.len(), 2);
        let nb_keys: Vec<&str> = slice
            .neighbor_sessions
            .iter()
            .map(|s| s.session_key.as_str())
            .collect();
        assert!(nb_keys.contains(&"nb-1"));
        assert!(nb_keys.contains(&"nb-2"));
    }

    #[test]
    fn neighborhood_slice_computes_shared_blockers() {
        let config = HotBrainConfig::default();
        let packets = vec![
            make_packet(
                "focal",
                vec!["db timeout", "auth down"],
                vec![],
                vec![],
                vec![],
                RiskLevel::Low,
            ),
            make_packet(
                "nb-1",
                vec!["db timeout", "disk full"],
                vec![],
                vec![],
                vec![],
                RiskLevel::Low,
            ),
        ];
        let relationships = vec![("focal".to_string(), "nb-1".to_string())];

        let slice = build_neighborhood_slice("focal", &packets, &[], &relationships, &config);

        assert!(
            slice.shared_blockers.contains(&"db timeout".to_string()),
            "'db timeout' should be a shared blocker"
        );
        assert!(
            !slice.shared_blockers.contains(&"auth down".to_string()),
            "'auth down' is only in focal, not shared"
        );
        assert!(
            !slice.shared_blockers.contains(&"disk full".to_string()),
            "'disk full' is only in neighbor, not shared"
        );
    }

    #[test]
    fn neighborhood_slice_limits_neighbors() {
        let config = HotBrainConfig::default();
        // Create 6 neighbor relationships
        let mut packets = vec![make_packet(
            "focal",
            vec![],
            vec![],
            vec![],
            vec![],
            RiskLevel::Low,
        )];
        let mut relationships = Vec::new();
        for i in 0..6 {
            let key = format!("nb-{}", i);
            packets.push(make_packet(
                &key,
                vec![],
                vec![],
                vec![],
                vec![],
                RiskLevel::Low,
            ));
            relationships.push(("focal".to_string(), key));
        }

        let slice = build_neighborhood_slice("focal", &packets, &[], &relationships, &config);

        assert!(
            slice.neighbor_sessions.len() <= 4,
            "max 4 neighbors, got {}",
            slice.neighbor_sessions.len()
        );
    }

    #[test]
    fn neighborhood_slice_tracks_confirmed_relations() {
        let config = HotBrainConfig::default();
        let packets = vec![
            make_packet("focal", vec![], vec![], vec![], vec![], RiskLevel::Low),
            make_packet("nb-1", vec![], vec![], vec![], vec![], RiskLevel::Low),
        ];
        let relationships = vec![("focal".to_string(), "nb-1".to_string())];

        let slice = build_neighborhood_slice("focal", &packets, &[], &relationships, &config);

        assert!(
            !slice.confirmed_relation_ids.is_empty(),
            "should track confirmed relation IDs"
        );
        assert!(
            slice.confirmed_relation_ids[0].contains("focal"),
            "relation ID should reference focal"
        );
    }
}
