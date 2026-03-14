/// Result of an AI summarization request.
#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub session_id: String,
    pub summary: String,
}

/// Result of an AI diagram generation request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagramResult {
    pub session_id: String,
    pub diagram: String,
    /// If true, this is a compact canvas-friendly diagram that should auto-add to canvas.
    #[serde(default)]
    pub canvas_compact: bool,
}

/// Result of a prompt behavior analysis request.
#[derive(Debug, Clone)]
pub struct BehaviorAnalysisResult {
    pub session_id: String,
    pub analysis: String,
}
