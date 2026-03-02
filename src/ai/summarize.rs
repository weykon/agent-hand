use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use ai_api_provider::{ApiClient, ApiConfig, ChatMessage, provider_by_name, resolve_api_key};

use crate::config::AiConfig;

/// Result of an AI summarization request.
#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub session_id: String,
    pub summary: String,
}

/// Background AI summarizer. Sends requests off the UI thread,
/// delivers results via channel.
pub struct Summarizer {
    _client: ApiClient,
    config: ApiConfig,
    /// Cached summaries keyed by session_id.
    cache: Arc<Mutex<HashMap<String, String>>>,
    /// Receives completed summaries.
    result_rx: mpsc::UnboundedReceiver<SummaryResult>,
    /// Sender cloned into spawned tasks.
    result_tx: mpsc::UnboundedSender<SummaryResult>,
    /// How many lines to capture. From config.
    pub capture_lines: usize,
}

impl Summarizer {
    /// Create from AI config section. Returns None if no provider/key available.
    pub fn from_config(ai_cfg: &AiConfig) -> Option<Self> {
        let meta = provider_by_name(&ai_cfg.provider)?;
        let api_key = if !ai_cfg.api_key.is_empty() {
            ai_cfg.api_key.clone()
        } else {
            resolve_api_key(meta.provider)?
        };

        let mut config = ApiConfig::new(meta.provider, api_key);
        if !ai_cfg.model.is_empty() {
            config.model = ai_cfg.model.clone();
        }
        config.base_url = ai_cfg.base_url.clone();
        // Summaries should be concise
        config.max_tokens = 1024;

        let (result_tx, result_rx) = mpsc::unbounded_channel();

        Some(Self {
            _client: ApiClient::new(),
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
            result_rx,
            result_tx,
            capture_lines: ai_cfg.summary_lines,
        })
    }

    /// Request a session summary in the background. Non-blocking.
    /// `pane_content` is the already-captured terminal output.
    pub fn summarize_session(&self, session_id: String, session_title: String, pane_content: String) {
        let config = self.config.clone();
        let client = ApiClient::new();
        let tx = self.result_tx.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            // Skip if already cached with same content hash
            {
                let c = cache.lock().await;
                if c.contains_key(&session_id) {
                    return;
                }
            }

            let messages = vec![
                ChatMessage::system(
                    "You are an assistant that summarizes terminal session output. \
                     Provide a concise 2-4 sentence summary of what this session is doing, \
                     its current state, and any notable output. \
                     Be specific about errors, completions, or pending actions. \
                     Reply in the same language as the terminal content."
                ),
                ChatMessage::user(format!(
                    "Session: {}\n\nTerminal output (most recent):\n```\n{}\n```",
                    session_title, pane_content
                )),
            ];

            match client.chat(&config, &messages).await {
                Ok(summary) => {
                    cache.lock().await.insert(session_id.clone(), summary.clone());
                    let _ = tx.send(SummaryResult { session_id, summary });
                }
                Err(e) => {
                    let error_msg = format!("AI error: {}", e);
                    let _ = tx.send(SummaryResult { session_id, summary: error_msg });
                }
            }
        });
    }

    /// Request a relationship context summary in the background.
    pub fn summarize_relationship(
        &self,
        relationship_id: String,
        session_a_title: String,
        session_a_content: String,
        session_b_title: String,
        session_b_content: String,
        relation_label: Option<String>,
    ) {
        let config = self.config.clone();
        let client = ApiClient::new();
        let tx = self.result_tx.clone();

        tokio::spawn(async move {
            let label_ctx = relation_label
                .map(|l| format!(" (relationship: {})", l))
                .unwrap_or_default();

            let messages = vec![
                ChatMessage::system(
                    "You analyze the relationship between two terminal sessions. \
                     Summarize what each session is doing and how they relate to each other. \
                     Identify shared context, dependencies, or collaboration points. \
                     Be specific and concise (3-5 sentences). \
                     Reply in the same language as the terminal content."
                ),
                ChatMessage::user(format!(
                    "Session A: {}{}\n```\n{}\n```\n\nSession B: {}\n```\n{}\n```",
                    session_a_title, label_ctx, session_a_content,
                    session_b_title, session_b_content
                )),
            ];

            match client.chat(&config, &messages).await {
                Ok(summary) => {
                    let _ = tx.send(SummaryResult {
                        session_id: relationship_id,
                        summary,
                    });
                }
                Err(e) => {
                    let _ = tx.send(SummaryResult {
                        session_id: relationship_id,
                        summary: format!("AI error: {}", e),
                    });
                }
            }
        });
    }

    /// Drain completed summaries (call from tick loop, non-blocking).
    pub fn poll_results(&mut self) -> Vec<SummaryResult> {
        let mut results = Vec::new();
        while let Ok(r) = self.result_rx.try_recv() {
            results.push(r);
        }
        results
    }

    /// Get cached summary for a session, if available.
    pub async fn get_cached(&self, session_id: &str) -> Option<String> {
        self.cache.lock().await.get(session_id).cloned()
    }

    /// Clear cache for a session (e.g. when content changes significantly).
    pub async fn invalidate(&self, session_id: &str) {
        self.cache.lock().await.remove(session_id);
    }

    /// Check if the summarizer has a valid provider configured.
    pub fn provider_name(&self) -> &str {
        ai_api_provider::provider_meta(self.config.provider).display_name
    }
}
