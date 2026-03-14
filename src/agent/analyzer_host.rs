//! HotBrainHost — analyzer orchestrator.
//!
//! Runs the built-in analyzer first, then each registered extension.
//! Merges outputs. Extension failures are logged but non-fatal.

use std::sync::Arc;

use super::analyzer::{Analyzer, BuiltInAnalyzer};
use super::hot_brain::{CandidateSet, CoordinationSlice, HotBrainConfig};

/// Orchestrates built-in + extension analyzers.
pub struct HotBrainHost {
    built_in: Arc<dyn Analyzer>,
    extensions: Vec<Arc<dyn Analyzer>>,
    config: HotBrainConfig,
}

impl HotBrainHost {
    pub fn new(config: HotBrainConfig) -> Self {
        let built_in = Arc::new(BuiltInAnalyzer::new(config.clone()));
        Self {
            built_in,
            extensions: Vec::new(),
            config,
        }
    }

    /// Register an extension analyzer.
    ///
    /// Extensions run after the built-in analyzer. Their outputs are merged
    /// into the final CandidateSet. Failures are logged but do not affect
    /// the built-in results.
    pub fn register_extension(&mut self, analyzer: Arc<dyn Analyzer>) {
        self.extensions.push(analyzer);
    }

    /// Run all analyzers and return merged output.
    ///
    /// Flow:
    /// 1. Run built-in analyzer (mandatory — failure here is an error)
    /// 2. Run each extension (failures logged, non-fatal)
    /// 3. Merge all outputs
    /// 4. Enforce config caps on merged output
    /// 5. Wrap in CandidateSet with trace_id/ts_ms
    pub fn analyze_all(
        &self,
        slice: &CoordinationSlice,
        trace_id: &str,
        ts_ms: u64,
    ) -> CandidateSet {
        // 1. Built-in (always succeeds for the deterministic built-in)
        let built_in_output = match self.built_in.analyze(slice, trace_id, ts_ms) {
            Ok(output) => output,
            Err(e) => {
                tracing::error!("Built-in analyzer failed (should not happen): {}", e);
                // Return empty candidate set on built-in failure
                return CandidateSet {
                    trace_id: trace_id.to_string(),
                    created_at_ms: ts_ms,
                    scheduler_hints: Vec::new(),
                    memory_candidates: Vec::new(),
                };
            }
        };

        let mut all_hints = built_in_output.scheduler_hints;
        let mut all_memories = built_in_output.memory_candidates;

        // 2. Extensions (non-fatal)
        for ext in &self.extensions {
            match ext.analyze(slice, trace_id, ts_ms) {
                Ok(output) => {
                    tracing::debug!(
                        "Extension '{}' v{} produced {} hints, {} memories",
                        ext.id(),
                        ext.version(),
                        output.scheduler_hints.len(),
                        output.memory_candidates.len()
                    );
                    all_hints.extend(output.scheduler_hints);
                    all_memories.extend(output.memory_candidates);
                }
                Err(e) => {
                    tracing::warn!(
                        "Extension '{}' v{} failed: {} — continuing with built-in results",
                        ext.id(),
                        ext.version(),
                        e
                    );
                }
            }
        }

        // 3. Enforce caps
        all_hints.truncate(self.config.max_scheduler_hints);
        all_memories.truncate(self.config.max_memory_candidates);

        // 4. Build final CandidateSet
        CandidateSet {
            trace_id: trace_id.to_string(),
            created_at_ms: ts_ms,
            scheduler_hints: all_hints,
            memory_candidates: all_memories,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::analyzer::{AnalyzerError, AnalyzerOutput};
    use crate::agent::guard::{self, RiskLevel};
    use crate::agent::hot_brain::{self, MemoryCandidate, MemoryCandidateKind, SchedulerHint, SchedulerHintKind};

    fn make_test_packet() -> guard::FeedbackPacket {
        guard::FeedbackPacket {
            packet_id: guard::short_id(),
            trace_id: "trace-host".to_string(),
            source_session_id: "sid-host".to_string(),
            goal: Some("test goal".to_string()),
            now: Some("testing".to_string()),
            done_this_turn: vec![],
            blockers: vec!["blocker-1".to_string()],
            decisions: vec!["decision-1".to_string()],
            findings: vec!["finding-1".to_string()],
            next_steps: vec!["step-1".to_string()],
            affected_targets: vec!["target-1".to_string()],
            source_refs: vec![],
            urgency_level: RiskLevel::Medium,
            recommended_response_level: guard::ResponseLevel::L2SelfInject,
            created_at_ms: 1700000000000,
        }
    }

    fn make_slice() -> (CoordinationSlice, HotBrainConfig) {
        let config = HotBrainConfig::default();
        let packets = vec![make_test_packet()];
        let slice = hot_brain::build_coordination_slice(&packets, &config);
        (slice, config)
    }

    /// Mock extension that produces fixed outputs.
    struct MockExtension {
        hints: Vec<SchedulerHint>,
        memories: Vec<MemoryCandidate>,
    }

    impl Analyzer for MockExtension {
        fn id(&self) -> &str { "mock-ext" }
        fn version(&self) -> &str { "0.1.0" }
        fn analyze(
            &self,
            _slice: &CoordinationSlice,
            _trace_id: &str,
            _ts_ms: u64,
        ) -> Result<AnalyzerOutput, AnalyzerError> {
            Ok(AnalyzerOutput {
                scheduler_hints: self.hints.clone(),
                memory_candidates: self.memories.clone(),
            })
        }
    }

    /// Mock extension that always fails.
    struct FailingExtension;

    impl Analyzer for FailingExtension {
        fn id(&self) -> &str { "failing-ext" }
        fn version(&self) -> &str { "0.0.1" }
        fn analyze(
            &self,
            _slice: &CoordinationSlice,
            _trace_id: &str,
            _ts_ms: u64,
        ) -> Result<AnalyzerOutput, AnalyzerError> {
            Err(AnalyzerError::Trap("intentional test failure".to_string()))
        }
    }

    #[test]
    fn host_no_extensions_matches_built_in() {
        let (slice, config) = make_slice();
        let trace_id = "trace-no-ext";
        let ts_ms = 1700000000000u64;

        // Direct via hot_brain
        let direct = hot_brain::analyze(&slice, &config, trace_id, ts_ms);

        // Via host with 0 extensions
        let host = HotBrainHost::new(config);
        let via_host = host.analyze_all(&slice, trace_id, ts_ms);

        assert_eq!(
            direct.scheduler_hints.len(),
            via_host.scheduler_hints.len()
        );
        assert_eq!(
            direct.memory_candidates.len(),
            via_host.memory_candidates.len()
        );

        for (d, h) in direct.scheduler_hints.iter().zip(via_host.scheduler_hints.iter()) {
            assert_eq!(d.hint_kind, h.hint_kind);
            assert_eq!(d.reason, h.reason);
        }
    }

    #[test]
    fn host_merges_extension_outputs() {
        let (slice, config) = make_slice();

        let ext_hint = SchedulerHint {
            id: guard::short_id(),
            trace_id: "trace-ext".to_string(),
            source_session_id: "sid-ext".to_string(),
            target_session_ids: vec!["sid-ext".to_string()],
            hint_kind: SchedulerHintKind::CoordinateNextSteps,
            reason: "extension hint".to_string(),
            urgency_level: RiskLevel::Low,
        };

        let ext_memory = MemoryCandidate {
            id: guard::short_id(),
            trace_id: "trace-ext".to_string(),
            source_session_id: "sid-ext".to_string(),
            kind: MemoryCandidateKind::Finding,
            summary: "extension finding".to_string(),
            source_refs: vec!["ext-ref".to_string()],
        };

        let mock = MockExtension {
            hints: vec![ext_hint.clone()],
            memories: vec![ext_memory.clone()],
        };

        let mut host = HotBrainHost::new(config.clone());
        let baseline = host.analyze_all(&slice, "t1", 1700000000000);
        let baseline_hints = baseline.scheduler_hints.len();
        let baseline_mems = baseline.memory_candidates.len();

        host.register_extension(Arc::new(mock));
        let merged = host.analyze_all(&slice, "t1", 1700000000000);

        // Should have more outputs (up to config cap)
        assert!(
            merged.scheduler_hints.len() >= baseline_hints,
            "merged should have at least as many hints"
        );
        assert!(
            merged.memory_candidates.len() >= baseline_mems,
            "merged should have at least as many memories"
        );
    }

    #[test]
    fn host_caps_merged_output() {
        let (slice, config) = make_slice();

        // Create extension with many outputs to exceed caps
        let many_hints: Vec<SchedulerHint> = (0..100)
            .map(|i| SchedulerHint {
                id: guard::short_id(),
                trace_id: "trace-ext".to_string(),
                source_session_id: format!("sid-{}", i),
                target_session_ids: vec![format!("sid-{}", i)],
                hint_kind: SchedulerHintKind::CoordinateNextSteps,
                reason: format!("hint {}", i),
                urgency_level: RiskLevel::Low,
            })
            .collect();

        let many_mems: Vec<MemoryCandidate> = (0..100)
            .map(|i| MemoryCandidate {
                id: guard::short_id(),
                trace_id: "trace-ext".to_string(),
                source_session_id: format!("sid-{}", i),
                kind: MemoryCandidateKind::Finding,
                summary: format!("finding {}", i),
                source_refs: vec!["ref".to_string()],
            })
            .collect();

        let mock = MockExtension {
            hints: many_hints,
            memories: many_mems,
        };

        let mut host = HotBrainHost::new(config.clone());
        host.register_extension(Arc::new(mock));
        let result = host.analyze_all(&slice, "t1", 1700000000000);

        assert!(
            result.scheduler_hints.len() <= config.max_scheduler_hints,
            "hints capped to {}, got {}",
            config.max_scheduler_hints,
            result.scheduler_hints.len()
        );
        assert!(
            result.memory_candidates.len() <= config.max_memory_candidates,
            "memories capped to {}, got {}",
            config.max_memory_candidates,
            result.memory_candidates.len()
        );
    }

    #[test]
    fn host_survives_extension_failure() {
        let (slice, config) = make_slice();

        let mut host = HotBrainHost::new(config);
        host.register_extension(Arc::new(FailingExtension));

        // Should NOT panic — failing extension is logged and skipped
        let result = host.analyze_all(&slice, "t1", 1700000000000);

        // Built-in results should still be present
        assert!(
            !result.scheduler_hints.is_empty() || !result.memory_candidates.is_empty(),
            "built-in results should survive extension failure"
        );
    }
}
