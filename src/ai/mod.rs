//! AI-powered features (Max tier).
//!
//! Provides session content summarization and relationship context analysis
//! using configurable LLM providers via the `ai_api_provider` crate.

mod summarize;

pub use summarize::Summarizer;
