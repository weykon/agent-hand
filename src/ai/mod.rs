//! AI-powered features (Max tier).
//!
//! Result types are always available for serialization/deserialization.
//! The `Summarizer` implementation lives in the private `pro` module and
//! is only available when compiled with `--features max`.

mod summarize;

pub use summarize::{BehaviorAnalysisResult, DiagramResult, SummaryResult};

// Re-export the real Summarizer from pro when max is enabled
#[cfg(feature = "max")]
pub use crate::pro::ai::Summarizer;
