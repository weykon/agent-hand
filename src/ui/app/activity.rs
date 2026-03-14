use std::time::Instant;

/// Identifies the type of async operation for dedup and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivityOp {
    CreatingSession,
    KillingSession,
    StartingSession,
    AttachingSession,
    SummarizingAI,
    GeneratingDiagram,
    TestingAIConnection,
    StartingShare,
    StoppingShare,
    RefreshingSessions,
    SavingData,
    CapturingPane,
}

impl ActivityOp {
    pub fn default_message(&self) -> &'static str {
        match self {
            Self::CreatingSession => "Creating session...",
            Self::KillingSession => "Stopping session...",
            Self::StartingSession => "Starting session...",
            Self::AttachingSession => "Attaching...",
            Self::SummarizingAI => "AI summarizing...",
            Self::GeneratingDiagram => "AI generating diagram...",
            Self::TestingAIConnection => "Testing AI connection...",
            Self::StartingShare => "Creating share link...",
            Self::StoppingShare => "Stopping share...",
            Self::RefreshingSessions => "Refreshing...",
            Self::SavingData => "Saving...",
            Self::CapturingPane => "Capturing output...",
        }
    }
}

/// A single active operation.
#[derive(Debug, Clone)]
pub struct Activity {
    pub op: ActivityOp,
    pub message: String,
    pub started_at: Instant,
}

/// Tracks all active async operations. Lives in App state.
#[derive(Debug, Default)]
pub struct ActivityTracker {
    active: Vec<Activity>,
}

impl ActivityTracker {
    /// Add an operation (deduplicates by op type).
    pub fn push(&mut self, op: ActivityOp, message: impl Into<String>) {
        // Remove existing op of same type to avoid duplicates
        self.active.retain(|a| a.op != op);
        self.active.push(Activity {
            op,
            message: message.into(),
            started_at: Instant::now(),
        });
    }

    /// Add an operation using its default message.
    pub fn push_default(&mut self, op: ActivityOp) {
        let msg = op.default_message().to_string();
        self.push(op, msg);
    }

    /// Remove a completed operation by type.
    pub fn complete(&mut self, op: ActivityOp) {
        self.active.retain(|a| a.op != op);
    }

    /// Whether a specific operation is currently active.
    pub fn is_active(&self, op: ActivityOp) -> bool {
        self.active.iter().any(|a| a.op == op)
    }

    /// The most recently pushed active operation (for rendering).
    pub fn current(&self) -> Option<&Activity> {
        self.active.last()
    }

    /// Whether any operations are active.
    pub fn has_any(&self) -> bool {
        !self.active.is_empty()
    }

    /// Remove operations older than 30 seconds (safety net for stuck ops).
    pub fn auto_expire(&mut self) {
        let cutoff = std::time::Duration::from_secs(30);
        self.active.retain(|a| a.started_at.elapsed() < cutoff);
    }
}
