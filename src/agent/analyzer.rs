//! Analyzer trait — pluggable Hot Brain analysis strategies.
//!
//! Defines the contract for analysis modules (built-in and WASM extensions)
//! and provides the default `BuiltInAnalyzer` that delegates to `hot_brain::analyze()`.

use serde::{Deserialize, Serialize};

use super::hot_brain::{self, CandidateSet, CoordinationSlice, HotBrainConfig, MemoryCandidate, SchedulerHint};

// ── Analyzer Output ─────────────────────────────────────────────────

/// Output from a single analyzer invocation.
///
/// Does NOT include trace_id/created_at_ms — those are per-pipeline metadata
/// added by the host when assembling the final `CandidateSet`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerOutput {
    pub scheduler_hints: Vec<SchedulerHint>,
    pub memory_candidates: Vec<MemoryCandidate>,
}

// ── Analyzer Error ──────────────────────────────────────────────────

/// Errors that an analyzer can produce.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzerError {
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("WASM trap: {0}")]
    Trap(String),
    #[error("analyzer timed out after {0}ms")]
    Timeout(u64),
}

// ── Analyzer Trait ──────────────────────────────────────────────────

/// A pluggable analysis strategy.
///
/// Analyzers are sync — `analyze()` is pure computation.
/// WASM invocation via wasmtime is also synchronous.
pub trait Analyzer: Send + Sync {
    /// Unique identifier for this analyzer (e.g. "built-in", "custom-scoring-v2").
    fn id(&self) -> &str;

    /// Semantic version string (e.g. "1.0.0").
    fn version(&self) -> &str;

    /// Run one analysis pass over a coordination slice.
    fn analyze(
        &self,
        slice: &CoordinationSlice,
        trace_id: &str,
        ts_ms: u64,
    ) -> Result<AnalyzerOutput, AnalyzerError>;
}

// ── Built-In Analyzer ───────────────────────────────────────────────

/// The default analyzer — wraps `hot_brain::analyze()`.
///
/// Always present; extensions are additive, never replacements.
pub struct BuiltInAnalyzer {
    pub config: HotBrainConfig,
}

impl BuiltInAnalyzer {
    pub fn new(config: HotBrainConfig) -> Self {
        Self { config }
    }
}

impl Analyzer for BuiltInAnalyzer {
    fn id(&self) -> &str {
        "built-in"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn analyze(
        &self,
        slice: &CoordinationSlice,
        trace_id: &str,
        ts_ms: u64,
    ) -> Result<AnalyzerOutput, AnalyzerError> {
        let candidate_set: CandidateSet =
            hot_brain::analyze(slice, &self.config, trace_id, ts_ms);
        Ok(AnalyzerOutput {
            scheduler_hints: candidate_set.scheduler_hints,
            memory_candidates: candidate_set.memory_candidates,
        })
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::guard;

    /// Helper: build a minimal FeedbackPacket for testing.
    fn make_test_packet(blockers: Vec<&str>, _urgency: &str) -> guard::FeedbackPacket {
        guard::FeedbackPacket {
            packet_id: guard::short_id(),
            trace_id: "trace-test".to_string(),
            source_session_id: "sid-test".to_string(),
            goal: Some("test goal".to_string()),
            now: Some("testing".to_string()),
            done_this_turn: vec![],
            blockers: blockers.into_iter().map(String::from).collect(),
            decisions: vec!["decision-1".to_string()],
            findings: vec!["finding-1".to_string()],
            next_steps: vec!["step-1".to_string()],
            affected_targets: vec!["target-1".to_string()],
            source_refs: vec![],
            urgency_level: guard::RiskLevel::Medium,
            recommended_response_level: guard::ResponseLevel::L2SelfInject,
            created_at_ms: 1700000000000,
        }
    }

    #[test]
    fn built_in_matches_direct_call() {
        let config = HotBrainConfig::default();
        let packets = vec![make_test_packet(vec!["db timeout"], "medium")];
        let slice = hot_brain::build_coordination_slice(&packets, &config);
        let trace_id = "trace-cmp";
        let ts_ms = 1700000000000u64;

        // Direct call
        let direct = hot_brain::analyze(&slice, &config, trace_id, ts_ms);

        // Via BuiltInAnalyzer
        let analyzer = BuiltInAnalyzer::new(config.clone());
        let via_trait = analyzer.analyze(&slice, trace_id, ts_ms).unwrap();

        // Outputs should match (ignoring generated IDs)
        assert_eq!(
            direct.scheduler_hints.len(),
            via_trait.scheduler_hints.len(),
            "hint count should match"
        );
        assert_eq!(
            direct.memory_candidates.len(),
            via_trait.memory_candidates.len(),
            "memory candidate count should match"
        );

        // Verify structural match: same hint kinds in same order
        for (d, t) in direct.scheduler_hints.iter().zip(via_trait.scheduler_hints.iter()) {
            assert_eq!(d.hint_kind, t.hint_kind);
            assert_eq!(d.source_session_id, t.source_session_id);
            assert_eq!(d.reason, t.reason);
        }

        // Verify structural match: same candidate kinds
        for (d, t) in direct.memory_candidates.iter().zip(via_trait.memory_candidates.iter()) {
            assert_eq!(d.kind, t.kind);
            assert_eq!(d.source_session_id, t.source_session_id);
        }
    }

    #[test]
    fn built_in_id_and_version() {
        let analyzer = BuiltInAnalyzer::new(HotBrainConfig::default());
        assert_eq!(analyzer.id(), "built-in");
        assert_eq!(analyzer.version(), "1.0.0");
    }
}
