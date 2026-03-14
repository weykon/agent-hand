# Security Mechanism — Evidence-Based Confirmation — SPEC

## 1. Overview

### Problem

AI coding agents (Claude Code, Cursor, Codex, etc.) operate with significant autonomy — editing files, running commands, installing packages, pushing code. The user **trusts but cannot verify** in real-time. Current failure modes include:

1. **Silent mistakes**: The agent modifies the wrong file, introduces a subtle bug, or misunderstands the intent. The user doesn't notice until much later.
2. **Omissions**: The agent solves part of the problem but skips edge cases, error handling, or related files that also need updating.
3. **Risky operations without gates**: A `git push --force`, `rm -rf`, or database migration runs without the user understanding the full impact.
4. **No audit trail**: After a session, there's no structured record of what the agent did, why, and what evidence supports each action.
5. **Compound errors**: An early mistake cascades through subsequent agent actions, each building on the flawed foundation.

### Solution

**Evidence-Based Confirmation (EBC)**: A security layer where AI agents must present structured evidence of their actions at configurable checkpoints. The system:

1. **Classifies operations by risk level** — low/medium/high/critical — with different confirmation requirements per level
2. **Requires evidence presentation** — what was changed, why, what was the reasoning, what could go wrong
3. **Proactively detects anomalies** — unusual file patterns, untested changes, scope creep, contradictory actions
4. **Integrates with hooks** — leverages the existing hook event system to intercept agent actions before and after execution
5. **Maintains an audit log** — structured, searchable record of all agent actions and their evidence

The mechanism is **not** about blocking agents or adding friction to every operation. It's about creating intelligent checkpoints at the right moments — when risk is high, when the user flagged something as important, or when the system detects something unusual.

### Design Principles

1. **Proportional friction**: Low-risk operations flow freely. High-risk operations require evidence. Critical operations require explicit confirmation.
2. **Evidence over permission**: Don't just ask "can I do X?" — present "I did X because Y, changing Z, with risk W."
3. **Proactive detection**: Don't wait for the user to notice problems. Detect patterns that suggest mistakes.
4. **Non-blocking by default**: The system should accelerate safe work, not slow it down.
5. **Composable**: Works with hooks, ECS events, viewer collaboration, and fork isolation.

---

## 2. User Stories

### US-1: Risk-gated file operations
> As a **user** running Claude Code, when the agent tries to delete a file or modify a config file in production paths, I want the system to pause and show me evidence of why this change is needed, so I can approve or reject it before it happens.

### US-2: Evidence summary after agent session
> As a **user** after a coding session with an AI agent, I want a structured summary of everything the agent did (files changed, commands run, packages installed), with the agent's reasoning for each action, so I can review the session.

### US-3: Proactive anomaly detection
> As a **user** who told the agent "fix the login bug," I want the system to flag if the agent also modified unrelated files (e.g., payment processing), so I catch scope creep before it becomes a problem.

### US-4: Difficult-issue escalation
> As a **user** working on a tricky concurrency bug, I want to mark the issue as "security-critical" so the agent presents more detailed evidence at each step, including what it considered and rejected.

### US-5: Compound error detection
> As a **user**, when the agent makes a change that breaks a test, then makes another change to "fix" the test by weakening the assertion, I want the system to detect this pattern and alert me that the agent may be papering over a real bug.

### US-6: Viewer security awareness
> As a **viewer** watching a host's AI agent session, I want to see the evidence confirmations in real-time, so I can spot issues the host might miss and flag them.

### US-7: Hook-triggered evidence
> As a **user** with custom hooks configured, I want certain hook events (e.g., `ToolFailure`, `PermissionRequest`) to automatically trigger evidence presentation, so the agent explains what went wrong and what it plans to do next.

### US-8: Lightweight audit for compliance
> As a **team lead**, I want an audit log of all agent actions across sessions with evidence records, so I can review agent behavior for compliance and quality.

---

## 3. Architecture

### 3.1 System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Agent Hand Process                                                         │
│                                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────────────────────┐ │
│  │  Hook Event   │────→│  EBC Engine  │────→│  Evidence Store              │ │
│  │  Receiver     │     │              │     │  (~/.agent-hand/evidence/)    │ │
│  │              │     │  1. Classify  │     └──────────────────────────────┘ │
│  │  (hook_events │     │  2. Analyze   │                                     │
│  │   .jsonl)     │     │  3. Gate      │     ┌──────────────────────────────┐ │
│  └──────────────┘     │  4. Record    │────→│  UI: Confirmation Dialog     │ │
│                        │              │     │  (when gate triggers)         │ │
│  ┌──────────────┐     │              │     └──────────────────────────────┘ │
│  │  PTY Monitor  │────→│  ↑           │                                     │
│  │  (terminal    │     │  │ Anomaly   │     ┌──────────────────────────────┐ │
│  │   output      │     │  │ Detector  │────→│  Toast Notifications         │ │
│  │   scanner)    │     │              │     │  (warnings, alerts)           │ │
│  └──────────────┘     └──────────────┘     └──────────────────────────────┘ │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────────┐│
│  │  AI Agent (Claude Code, Cursor, etc.) — running in tmux pane            ││
│  │  [file edits] [shell commands] [git operations] [package installs]      ││
│  └──────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Risk Classification Engine

Operations are classified into four risk levels. Classification uses a **rule-based system** with configurable overrides.

```
Risk Classification Pipeline:

  Hook Event / Terminal Output
           │
           ▼
  ┌─────────────────┐
  │  Pattern Matcher │  ← matches against known operation patterns
  │                   │
  │  "rm -rf /"      │ → CRITICAL
  │  "git push -f"   │ → HIGH
  │  "npm install X"  │ → MEDIUM
  │  "edit src/foo.rs"│ → LOW
  └────────┬──────────┘
           │
           ▼
  ┌─────────────────┐
  │  Context Scorer  │  ← adjusts based on context
  │                   │
  │  + production?   │  → risk += 1 level
  │  + config file?  │  → risk += 1 level
  │  + user flagged? │  → risk = max(current, HIGH)
  │  + test file?    │  → risk -= 1 level
  └────────┬──────────┘
           │
           ▼
  ┌─────────────────┐
  │  Gate Decision   │
  │                   │
  │  LOW:    log only │
  │  MEDIUM: evidence │
  │  HIGH:   confirm  │
  │  CRITICAL: block  │
  └───────────────────┘
```

### 3.3 Evidence Presentation Model

Evidence is structured data that captures the **what, why, and risk** of an agent action.

```
┌──────────────────────────────────────────────────────────────────┐
│  EVIDENCE RECORD                                                  │
│                                                                    │
│  ┌─────────────────────────────────┐                              │
│  │  Action                          │  What the agent did/wants   │
│  │  "Modified src/auth/login.rs"    │  to do                      │
│  └─────────────────────────────────┘                              │
│                                                                    │
│  ┌─────────────────────────────────┐                              │
│  │  Reasoning                       │  Why the agent chose this   │
│  │  "Login function didn't handle   │  action                     │
│  │   expired tokens, added check"   │                              │
│  └─────────────────────────────────┘                              │
│                                                                    │
│  ┌─────────────────────────────────┐                              │
│  │  Changes                         │  Specific modifications     │
│  │  - Added: token_expired() check  │                              │
│  │  - Modified: login() return type │                              │
│  │  - Affected: 2 files, 34 lines   │                              │
│  └─────────────────────────────────┘                              │
│                                                                    │
│  ┌─────────────────────────────────┐                              │
│  │  Risk Assessment                 │  What could go wrong        │
│  │  Level: MEDIUM                   │                              │
│  │  Factors: auth code, session mgmt│                              │
│  │  Mitigations: existing tests pass│                              │
│  └─────────────────────────────────┘                              │
│                                                                    │
│  ┌─────────────────────────────────┐                              │
│  │  Alternatives Considered         │  What else was evaluated    │
│  │  - Could modify middleware       │  (for HIGH+ risk)           │
│  │  - Could add separate validator  │                              │
│  │  - Chose inline check for        │                              │
│  │    simplicity                     │                              │
│  └─────────────────────────────────┘                              │
└──────────────────────────────────────────────────────────────────┘
```

### 3.4 Anomaly Detection Subsystem

The anomaly detector runs passively, analyzing patterns across agent actions within a session.

```
Anomaly Detection Patterns:

1. SCOPE CREEP
   Signal: Agent modifies files outside the apparent task scope
   Example: Asked to fix login → modifies payment.rs
   Detection: Track "task scope" from initial prompt, flag unrelated file paths

2. TEST WEAKENING
   Signal: Agent modifies test assertions after a test failure
   Example: Changes assert_eq!(result, 42) to assert!(result > 0)
   Detection: Correlate ToolFailure events with subsequent test file edits

3. ERROR MASKING
   Signal: Agent catches/suppresses errors instead of fixing root cause
   Example: Wraps failing code in try/catch with empty handler
   Detection: Pattern match on catch blocks with no meaningful handling

4. CIRCULAR CHANGES
   Signal: Agent reverts or re-does a previous change
   Example: Edit A → Edit B → Revert A → Edit A again
   Detection: Track file change history within session, detect revisits

5. DEPENDENCY INFLATION
   Signal: Agent adds heavy dependencies for simple tasks
   Example: Adds 3 new packages to fix a string formatting issue
   Detection: Track package.json/Cargo.toml changes relative to task complexity

6. CONFIGURATION DRIFT
   Signal: Agent modifies environment or config files unexpectedly
   Example: Changes .env, Dockerfile, CI pipeline without being asked
   Detection: Flag config file modifications that weren't in the original request
```

---

## 4. Protocol / API

### 4.1 Core Data Structures

```rust
/// Risk level for an agent operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Routine operations: reading files, running tests, editing non-critical code
    /// Gate: Log only, no user interaction
    Low = 0,

    /// Moderate operations: installing packages, modifying shared code, git commits
    /// Gate: Present evidence in status bar, user can review
    Medium = 1,

    /// Significant operations: modifying auth/security code, config changes, git push
    /// Gate: Confirmation dialog required before proceeding
    High = 2,

    /// Dangerous operations: force push, file deletion, production deploys, DB migrations
    /// Gate: Blocking confirmation with full evidence + explicit "I understand" acknowledgment
    Critical = 3,
}

/// A structured evidence record for an agent action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Unique identifier
    pub id: String,

    /// Session this evidence belongs to
    pub session_id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// The action taken or proposed
    pub action: ActionDescription,

    /// Agent's reasoning for this action
    pub reasoning: String,

    /// Specific changes made or proposed
    pub changes: Vec<ChangeDetail>,

    /// Computed risk assessment
    pub risk: RiskAssessment,

    /// Alternatives the agent considered (populated for HIGH+ risk)
    pub alternatives: Vec<Alternative>,

    /// User's decision (if gate was triggered)
    pub decision: Option<UserDecision>,

    /// Any anomalies detected in conjunction with this action
    pub anomalies: Vec<Anomaly>,

    /// Source hook event that triggered this record (if any)
    pub source_event: Option<HookEventKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescription {
    /// Human-readable summary
    pub summary: String,

    /// Category of action
    pub category: ActionCategory,

    /// Files involved
    pub files: Vec<PathBuf>,

    /// Commands involved (if any)
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionCategory {
    FileEdit,
    FileCreate,
    FileDelete,
    ShellCommand,
    GitOperation,
    PackageInstall,
    ConfigChange,
    TestModification,
    BuildOperation,
    DeployOperation,
    DatabaseOperation,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetail {
    /// File path affected
    pub file: PathBuf,

    /// Type of change
    pub change_type: ChangeType,

    /// Lines added
    pub lines_added: usize,

    /// Lines removed
    pub lines_removed: usize,

    /// Brief description of what changed
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
    Permissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Computed risk level
    pub level: RiskLevel,

    /// Factors that contributed to this risk level
    pub factors: Vec<RiskFactor>,

    /// Mitigating factors (e.g., tests pass, non-production)
    pub mitigations: Vec<String>,

    /// Numeric score (0.0 - 1.0) for fine-grained sorting
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub factor: String,
    pub weight: f32,
    /// How much this factor raised the risk
    pub contribution: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub description: String,
    pub reason_rejected: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserDecision {
    /// User approved the action
    Approved { at: DateTime<Utc> },
    /// User rejected the action
    Rejected { at: DateTime<Utc>, reason: Option<String> },
    /// User requested more information
    RequestedDetail { at: DateTime<Utc>, question: String },
    /// Auto-approved (LOW risk, no gate)
    AutoApproved,
    /// Timed out waiting for user response
    TimedOut { at: DateTime<Utc> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Type of anomaly detected
    pub pattern: AnomalyPattern,
    /// Human-readable description
    pub description: String,
    /// Severity (how concerning is this?)
    pub severity: AnomalySeverity,
    /// Related evidence records
    pub related_records: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyPattern {
    ScopeCreep,
    TestWeakening,
    ErrorMasking,
    CircularChanges,
    DependencyInflation,
    ConfigurationDrift,
    UntestedChanges,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Info,
    Warning,
    Alert,
}
```

### 4.2 Risk Classification Rules

```rust
/// Default risk classification rules
/// Users can override these in config.json
pub struct RiskClassifier {
    rules: Vec<ClassificationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRule {
    /// Pattern to match against (glob or regex)
    pub pattern: String,
    /// What the pattern matches against
    pub match_target: MatchTarget,
    /// Base risk level when matched
    pub risk_level: RiskLevel,
    /// Optional: override description
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchTarget {
    /// Match against file paths
    FilePath,
    /// Match against shell commands
    Command,
    /// Match against git operations
    GitOperation,
    /// Match against hook event kind
    HookEvent,
}

/// Built-in default rules
const DEFAULT_RULES: &[(&str, MatchTarget, RiskLevel)] = &[
    // CRITICAL
    ("rm -rf *",                    MatchTarget::Command,      RiskLevel::Critical),
    ("git push --force*",           MatchTarget::Command,      RiskLevel::Critical),
    ("git push -f*",                MatchTarget::Command,      RiskLevel::Critical),
    ("git reset --hard*",           MatchTarget::Command,      RiskLevel::Critical),
    ("drop table*",                 MatchTarget::Command,      RiskLevel::Critical),
    ("DROP TABLE*",                 MatchTarget::Command,      RiskLevel::Critical),
    ("*/migrations/*.sql",          MatchTarget::FilePath,     RiskLevel::Critical),

    // HIGH
    ("git push*",                   MatchTarget::Command,      RiskLevel::High),
    ("*/.env*",                     MatchTarget::FilePath,     RiskLevel::High),
    ("*/secrets*",                  MatchTarget::FilePath,     RiskLevel::High),
    ("*/credentials*",              MatchTarget::FilePath,     RiskLevel::High),
    ("*Dockerfile*",                MatchTarget::FilePath,     RiskLevel::High),
    ("*docker-compose*",            MatchTarget::FilePath,     RiskLevel::High),
    ("*.github/workflows/*",        MatchTarget::FilePath,     RiskLevel::High),
    ("*/auth/*",                    MatchTarget::FilePath,     RiskLevel::High),
    ("*/security/*",                MatchTarget::FilePath,     RiskLevel::High),
    ("chmod*",                      MatchTarget::Command,      RiskLevel::High),

    // MEDIUM
    ("*/package.json",              MatchTarget::FilePath,     RiskLevel::Medium),
    ("*/Cargo.toml",                MatchTarget::FilePath,     RiskLevel::Medium),
    ("*/config*",                   MatchTarget::FilePath,     RiskLevel::Medium),
    ("npm install*",                MatchTarget::Command,      RiskLevel::Medium),
    ("cargo add*",                  MatchTarget::Command,      RiskLevel::Medium),
    ("pip install*",                MatchTarget::Command,      RiskLevel::Medium),
    ("git commit*",                 MatchTarget::Command,      RiskLevel::Medium),
    ("git merge*",                  MatchTarget::Command,      RiskLevel::Medium),

    // LOW (everything else defaults to LOW)
];
```

### 4.3 Context Scoring Modifiers

```rust
/// Context-aware risk adjustment
pub struct ContextScorer;

impl ContextScorer {
    /// Adjust risk level based on contextual signals
    pub fn adjust(base: RiskLevel, context: &ActionContext) -> RiskLevel {
        let mut level = base;

        // Escalate for production-related paths
        if context.path_contains(&["prod", "production", "deploy"]) {
            level = level.escalate();
        }

        // Escalate for user-flagged sessions
        if context.session_security_level == SecurityLevel::Critical {
            level = level.max(RiskLevel::High);
        }

        // De-escalate for test files (unless test weakening detected)
        if context.is_test_file && !context.anomaly_detected {
            level = level.deescalate();
        }

        // Escalate when agent has had recent failures
        if context.recent_failures > 2 {
            level = level.escalate();
        }

        level
    }
}
```

### 4.4 EBC Engine API

```rust
pub struct EbcEngine {
    /// Risk classifier with rules
    classifier: RiskClassifier,
    /// Context scorer
    context_scorer: ContextScorer,
    /// Anomaly detector
    anomaly_detector: AnomalyDetector,
    /// Evidence store
    store: EvidenceStore,
    /// Session-level configuration
    config: EbcConfig,
    /// Action history for anomaly detection
    session_history: Vec<EvidenceRecord>,
}

impl EbcEngine {
    /// Process a hook event through the EBC pipeline
    /// Returns: gate decision (what UI action to take)
    pub async fn process_event(
        &mut self,
        event: &HookEvent,
        session: &Instance,
    ) -> GateDecision;

    /// Process a terminal output observation (from PTY monitor)
    pub async fn process_terminal_output(
        &mut self,
        output: &str,
        session: &Instance,
    ) -> Option<GateDecision>;

    /// User escalates session security level
    pub fn set_session_security(&mut self, session_id: &str, level: SecurityLevel);

    /// Generate session summary with all evidence
    pub fn session_summary(&self, session_id: &str) -> SessionEvidenceSummary;

    /// Export audit log for a time range
    pub fn export_audit_log(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<EvidenceRecord>;
}

#[derive(Debug, Clone)]
pub enum GateDecision {
    /// No action needed, log only
    Pass {
        record: EvidenceRecord,
    },

    /// Show evidence in status bar / toast (user can review but isn't blocked)
    Inform {
        record: EvidenceRecord,
        message: String,
    },

    /// Require user confirmation before proceeding
    Confirm {
        record: EvidenceRecord,
        dialog: ConfirmationDialog,
    },

    /// Block the action — requires explicit acknowledgment with understanding
    Block {
        record: EvidenceRecord,
        dialog: BlockDialog,
        timeout_seconds: u32,
    },

    /// Anomaly detected — alert the user
    Alert {
        anomaly: Anomaly,
        related_records: Vec<EvidenceRecord>,
        message: String,
    },
}
```

### 4.5 Anomaly Detector API

```rust
pub struct AnomalyDetector {
    /// Patterns to detect
    patterns: Vec<AnomalyPatternConfig>,
    /// Session action history (sliding window)
    history: VecDeque<ActionSummary>,
    /// Known task scope (files/directories the user asked about)
    task_scope: Option<TaskScope>,
}

impl AnomalyDetector {
    /// Analyze a new action against session history
    pub fn check(&mut self, action: &ActionDescription) -> Vec<Anomaly>;

    /// Set the task scope (derived from user's initial prompt or explicit declaration)
    pub fn set_scope(&mut self, scope: TaskScope);

    /// Detect scope creep: action touches files outside task scope
    fn check_scope_creep(&self, action: &ActionDescription) -> Option<Anomaly>;

    /// Detect test weakening: test assertion changed after test failure
    fn check_test_weakening(&self, action: &ActionDescription) -> Option<Anomaly>;

    /// Detect circular changes: same file modified, reverted, modified again
    fn check_circular_changes(&self, action: &ActionDescription) -> Option<Anomaly>;

    /// Detect error masking: catch blocks with empty/minimal handlers added
    fn check_error_masking(&self, action: &ActionDescription) -> Option<Anomaly>;

    /// Detect dependency inflation: many packages added for a simple task
    fn check_dependency_inflation(&self, action: &ActionDescription) -> Option<Anomaly>;

    /// Detect config drift: config files modified without being in scope
    fn check_config_drift(&self, action: &ActionDescription) -> Option<Anomaly>;
}

#[derive(Debug, Clone)]
pub struct TaskScope {
    /// Files/directories the task is expected to touch
    pub expected_paths: Vec<PathBuf>,
    /// Keywords from the user's initial prompt
    pub keywords: Vec<String>,
    /// How strictly to enforce scope (loose allows adjacent files)
    pub strictness: ScopeStrictness,
}

#[derive(Debug, Clone)]
pub enum ScopeStrictness {
    /// Only flag files completely outside the project area
    Loose,
    /// Flag files outside the expected module/directory
    Normal,
    /// Flag any file not explicitly in expected_paths
    Strict,
}
```

### 4.6 Hook Integration

The EBC engine connects to the existing hook event system:

```rust
/// Extended hook event processing
impl EbcEngine {
    /// Map hook events to EBC evidence records
    pub fn hook_to_evidence(&self, event: &HookEvent) -> Option<EvidenceRecord> {
        match &event.kind {
            HookEventKind::PermissionRequest => {
                // Agent is asking for permission — create evidence for what it wants to do
                Some(self.create_permission_evidence(event))
            }
            HookEventKind::ToolFailure => {
                // Agent tool failed — record the failure and check for patterns
                Some(self.create_failure_evidence(event))
            }
            HookEventKind::Stop => {
                // Agent stopped — generate session summary
                self.generate_session_summary(event);
                None
            }
            HookEventKind::Notification => {
                // Agent notification — check if it contains evidence-relevant info
                self.check_notification_for_evidence(event)
            }
            _ => None,
        }
    }
}
```

### 4.7 Storage Schema

Evidence records are stored per-session in a dedicated directory:

```
~/.agent-hand/
  evidence/
    {session_id}/
      evidence.jsonl        ← append-only evidence records
      summary.json          ← session summary (generated on session end)
      anomalies.jsonl       ← detected anomalies
    audit/
      {YYYY-MM-DD}.jsonl    ← daily audit log (aggregated across sessions)
```

---

## 5. UI/UX Design

### 5.1 Risk Level Visual Indicators

Each risk level has a distinct visual treatment in the TUI:

```
Risk Level    Color      Icon    Status Bar               Gate Behavior
──────────    ─────      ────    ──────────               ─────────────
LOW           dim gray   ·       (not shown)              Log silently
MEDIUM        yellow     ⚡      "⚡ npm install lodash"   Toast (3s)
HIGH          orange     ⚠       "⚠ Modifying auth code"  Confirmation dialog
CRITICAL      red+bold   🛑      "🛑 FORCE PUSH BLOCKED"  Blocking dialog
```

### 5.2 Evidence Toast (MEDIUM Risk)

Non-blocking notification that appears for a few seconds:

```
┌──────────────────────────────────────────────────────────────────────┐
│  [terminal content...]                                               │
│                                                                      │
│                                                                      │
│──────────────────────────────────────────────────────────────────────│
│  ⚡ Agent installed lodash (MEDIUM)                    [r] Review    │
└──────────────────────────────────────────────────────────────────────┘
```

Pressing `r` expands the evidence detail:

```
┌──────────────── Evidence Detail ─────────────────┐
│                                                    │
│  Action: npm install lodash                        │
│  Reasoning: "Need deep clone utility for          │
│    merging config objects"                          │
│                                                    │
│  Changes:                                          │
│    + package.json: added lodash@4.17.21            │
│    + package-lock.json: 15 new entries             │
│                                                    │
│  Risk: MEDIUM                                      │
│    Factor: New dependency added                    │
│    Mitigation: Well-known stable package           │
│                                                    │
│  [OK]                                              │
└────────────────────────────────────────────────────┘
```

### 5.3 Confirmation Dialog (HIGH Risk)

Blocking dialog that requires explicit user input:

```
┌───────────────── ⚠ Confirmation Required ─────────────────┐
│                                                             │
│  Action: Modify src/auth/login.rs                           │
│                                                             │
│  Reasoning:                                                 │
│    "Login function doesn't handle expired tokens.           │
│     Added expiry check before token validation."            │
│                                                             │
│  Changes:                                                   │
│    ~ src/auth/login.rs: +12 lines, -3 lines                │
│      - Added token_expired() check                          │
│      - Modified login() return type to Result<>             │
│    ~ src/auth/mod.rs: +1 line                               │
│      - Re-exported TokenExpiredError                        │
│                                                             │
│  Risk: HIGH                                                 │
│    Factors: authentication code, error handling change       │
│    Mitigations: existing test suite covers login flow        │
│                                                             │
│  Alternatives Considered:                                   │
│    1. Middleware-level check (rejected: too broad)           │
│    2. Separate validator (rejected: over-engineered)         │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌────────────┐               │
│  │ Approve  │  │  Reject  │  │ More Info  │               │
│  └──────────┘  └──────────┘  └────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

### 5.4 Blocking Dialog (CRITICAL Risk)

Full-screen blocking dialog with explicit acknowledgment:

```
┌─────────────────────── 🛑 CRITICAL ACTION ───────────────────────────┐
│                                                                       │
│  ██  FORCE PUSH DETECTED  ██                                          │
│                                                                       │
│  Action: git push --force origin main                                 │
│                                                                       │
│  ⚠ This will OVERWRITE remote history on the main branch.            │
│  ⚠ Other team members' work may be PERMANENTLY LOST.                 │
│                                                                       │
│  Reasoning:                                                           │
│    "Rebased main to remove accidental commit with credentials"        │
│                                                                       │
│  Impact:                                                              │
│    - 3 commits on remote will be replaced                            │
│    - Affects: main branch (default/protected)                        │
│    - Other contributors may need to force-pull                       │
│                                                                       │
│  Safer Alternatives:                                                  │
│    1. git revert <commit> (preserves history)                         │
│    2. git push --force-with-lease (prevents overwriting others)       │
│                                                                       │
│  To proceed, type "FORCE PUSH" and press Enter:                      │
│  > _                                                                  │
│                                                                       │
│  [C]ancel   [S]uggest alternative                                    │
└───────────────────────────────────────────────────────────────────────┘
```

### 5.5 Anomaly Alert

When the anomaly detector flags something:

```
┌──────────────── ⚡ Anomaly Detected ────────────────┐
│                                                       │
│  Pattern: SCOPE CREEP                                 │
│                                                       │
│  You asked the agent to "fix the login bug"           │
│  but the agent also modified:                         │
│                                                       │
│    src/payments/checkout.rs  (+15 lines)              │
│    src/api/routes.rs         (+8 lines)               │
│                                                       │
│  These files appear unrelated to the login system.    │
│                                                       │
│  ┌──────────┐  ┌──────────────┐  ┌──────────┐       │
│  │ Review   │  │ It's fine ✓  │  │ Revert   │       │
│  └──────────┘  └──────────────┘  └──────────┘       │
└───────────────────────────────────────────────────────┘
```

### 5.6 Session Evidence Summary

Accessible via keybinding (`Ctrl+E`) on a session in the list:

```
┌────────────── Session Evidence Summary ──────────────┐
│                                                        │
│  Session: myapp (Claude Code)                          │
│  Duration: 45 minutes                                  │
│  Total Actions: 23                                     │
│                                                        │
│  Risk Distribution:                                    │
│    LOW: 18  ████████████████████                       │
│    MEDIUM: 3  ████                                     │
│    HIGH: 2  ███                                        │
│    CRITICAL: 0                                         │
│                                                        │
│  Anomalies: 1 (Scope Creep — dismissed by user)       │
│                                                        │
│  Files Modified: 7                                     │
│  Lines Added: 142  │  Lines Removed: 38               │
│  Commands Run: 12                                      │
│  Packages Installed: 1 (lodash)                       │
│                                                        │
│  [D]etail view  [E]xport  [C]lose                     │
└────────────────────────────────────────────────────────┘
```

### 5.7 Keybindings

| Context | Key | Action |
|---------|-----|--------|
| Session list | `Ctrl+E` | View evidence summary for selected session |
| Session list | `Ctrl+S` | Set session security level (Low/Normal/High/Critical) |
| Evidence toast | `r` | Review evidence detail |
| Confirmation dialog | `Enter` / `a` | Approve |
| Confirmation dialog | `Esc` / `x` | Reject |
| Confirmation dialog | `m` | Request more info |
| Blocking dialog | (type phrase) | Confirm critical action |
| Anomaly alert | `Enter` | Review changes |
| Anomaly alert | `f` | Dismiss ("it's fine") |
| Anomaly alert | `u` | Revert changes |

### 5.8 Configuration (config.json)

```json
{
  "security": {
    "enabled": true,
    "default_session_level": "normal",
    "gate_levels": {
      "low": "log",
      "medium": "inform",
      "high": "confirm",
      "critical": "block"
    },
    "anomaly_detection": true,
    "evidence_retention_days": 30,
    "audit_log": true,
    "toast_duration_seconds": 5,
    "confirmation_timeout_seconds": 300,
    "custom_rules": [],
    "scope_strictness": "normal",
    "auto_summary_on_stop": true
  }
}
```

---

## 6. Implementation Strategy

### Phase 1: Risk Classification & Evidence Recording

**Goal**: Every agent action is classified by risk and recorded. No UI gates yet — log only.

1. **Risk classifier** (`src/security/classifier.rs`)
   - `RiskClassifier` with default rules
   - `ClassificationRule` matching against file paths, commands, git operations
   - `ContextScorer` for context-aware adjustment
   - Unit tests for classification accuracy

2. **Evidence data structures** (`src/security/evidence.rs`)
   - `EvidenceRecord`, `RiskAssessment`, `ActionDescription`, `ChangeDetail`
   - Serialization/deserialization
   - Builder pattern for constructing evidence records

3. **Evidence store** (`src/security/store.rs`)
   - Append-only JSONL writing to `~/.agent-hand/evidence/{session_id}/`
   - Session summary generation
   - Retention/cleanup for old evidence

4. **Hook integration** (`src/security/hook_bridge.rs`)
   - Connect `EventReceiver` to `EbcEngine`
   - Map `HookEventKind` variants to evidence records
   - Run classification on each hook event

**Deliverable**: Every hook event from AI agents is classified, evidence is recorded to disk. `evidence.jsonl` files accumulate per session.

### Phase 2: UI Gates & Confirmation

**Goal**: Risk-appropriate UI gates activate for MEDIUM+ operations.

1. **Toast notifications** for MEDIUM risk
   - Non-blocking evidence summary in status bar
   - `r` key to expand detail view
   - Auto-dismiss after configurable timeout

2. **Confirmation dialog** for HIGH risk
   - Blocking dialog with evidence presentation
   - Approve/Reject/More Info actions
   - Record user decision in evidence

3. **Blocking dialog** for CRITICAL risk
   - Full-screen dialog with explicit phrase confirmation
   - Suggest safer alternatives
   - Cannot be dismissed without action

4. **Session evidence summary** (`Ctrl+E`)
   - Aggregate view of all evidence for a session
   - Risk distribution chart
   - File/command/package summary

**Deliverable**: Users see and interact with evidence at appropriate risk levels. Decisions are recorded.

### Phase 3: Anomaly Detection

**Goal**: Proactive detection of agent mistakes and suspicious patterns.

1. **Anomaly detector** (`src/security/anomaly.rs`)
   - Session history tracking (sliding window of recent actions)
   - Pattern implementations: scope creep, test weakening, circular changes, error masking, dependency inflation, config drift

2. **Task scope inference**
   - Derive scope from the initial user prompt (parsed from hook events)
   - Allow explicit scope declaration via session security settings
   - Adjustable strictness (loose/normal/strict)

3. **Anomaly alerting**
   - Alert UI when anomaly detected
   - Options: review, dismiss, revert
   - Anomaly history in evidence store

**Deliverable**: The system proactively detects common agent mistakes and alerts the user.

### Phase 4: Terminal Output Analysis

**Goal**: Analyze raw terminal output for risk signals beyond hook events.

1. **PTY output scanner** (`src/security/pty_scanner.rs`)
   - Regex-based pattern matching against terminal output
   - Detect commands as they're typed (before execution)
   - Detect error messages, stack traces, permission denials

2. **Pre-execution gating**
   - For critical commands detected in terminal output, insert a gate before the command executes
   - Requires coordination with the PTY pipeline

3. **Output-based evidence enrichment**
   - Correlate terminal output with hook events for richer evidence
   - Detect success/failure of operations from output

**Deliverable**: Terminal output is analyzed for risk signals, providing an additional layer of detection beyond hook events.

### Phase 5: Audit & Export

**Goal**: Compliance-ready audit trail.

1. **Daily audit log aggregation** (`~/.agent-hand/evidence/audit/`)
   - Aggregate evidence across sessions into daily logs
   - Configurable retention period

2. **Export functionality**
   - Export evidence as JSON, CSV, or Markdown
   - Filter by date range, session, risk level

3. **CLI commands**
   - `agent-hand evidence <session-id>` — view evidence for a session
   - `agent-hand audit --from 2026-03-01 --to 2026-03-06` — audit log
   - `agent-hand audit --export csv` — export for compliance

**Deliverable**: Structured audit trail that can be exported and reviewed.

---

## 7. Dependencies

### Depends On
- **Existing hook system** (`src/hooks/`): EBC is primarily driven by hook events. The `EventReceiver` and `HookEventKind` are the primary input.
- **Existing session management** (`src/session/`): Evidence is organized per session.
- **Existing tmux integration** (`src/tmux/`): PTY output scanning requires access to tmux pane content.

### Enables
- **SPEC-02 (Guest Fork)**: Fork operations are classifiable by risk level. Creating a worktree = MEDIUM, revoking a fork = LOW.
- **SPEC-03 (Presence & Cursor Tracking)**: Viewers can see evidence confirmations in real-time.
- **SPEC-05 (ECS Runtime)**: Evidence records are natural ECS components. `EvidenceRecord` can be an ECS entity with risk, action, and decision components.
- **SPEC-07 (Memory & Relationships)**: Evidence history informs relationship context between sessions.

### External Dependencies
- No new crate dependencies for Phase 1-3. Uses existing `serde`, `chrono`, `regex`.
- Phase 4 (PTY scanning) may benefit from `regex` crate optimizations for high-throughput matching.

---

## 8. Performance Considerations

### 8.1 Non-Blocking Pipeline

The EBC engine must not block the TUI event loop:

```
Hook Event arrives
       │
       ▼
  Classify (< 1ms, CPU only, no I/O)
       │
       ▼
  Write evidence to JSONL (async, buffered I/O)
       │
       ▼
  Gate decision
       │
       ├── Pass/Inform → non-blocking, TUI continues
       └── Confirm/Block → dialog shown, TUI paused (user action required)
```

### 8.2 Anomaly Detection Cost

- History window: last 50 actions (configurable)
- Pattern checks: O(n) per action where n = history size
- Total cost per action: < 5ms
- Can be debounced for rapid-fire events (batch 10ms)

### 8.3 Storage Efficiency

- Evidence JSONL: ~200-500 bytes per record
- Typical session (30 min): ~50-100 records = ~25-50 KB
- Audit log: ~1-5 MB per day for active usage
- Retention: 30 days default = ~30-150 MB total

---

## 9. Open Questions

### Q1: How to extract evidence from AI agents?
Current hook events provide limited information (event kind, session, cwd). Richer evidence (reasoning, alternatives considered) would require deeper integration with the AI agent. **Proposed approach**: Start with what hooks provide. For agents that support it (e.g., Claude Code's notification hooks), extract richer context. For others, infer from terminal output.

### Q2: Pre-execution vs post-execution gating?
Should HIGH/CRITICAL gates block the action before it executes, or present evidence after? Pre-execution is safer but requires intercepting the agent's actions (which may not be possible for all agents). Post-execution with revert capability is more universally applicable. **Recommendation**: Post-execution for Phase 1-3 (evidence and alerting), pre-execution for Phase 4 (PTY scanning can detect commands before Enter is pressed).

### Q3: User fatigue from false positives?
If the anomaly detector generates too many false positives, users will disable it. **Mitigation**: Start with conservative thresholds. Track dismiss rates. If a pattern is dismissed >80% of the time by a user, suggest disabling that specific pattern.

### Q4: Multi-agent sessions?
When multiple AI agents are running in different sessions, should the EBC engine correlate actions across sessions? For example, two agents modifying the same file. **Recommendation**: Phase 1 is per-session. Cross-session analysis can be added later using the audit log as input.

### Q5: Evidence for viewer collaboration?
Should viewers see the host's evidence confirmations in real-time? This adds valuable oversight but also reveals the host's decision-making process. **Recommendation**: Show evidence toasts and anomaly alerts to viewers. Confirmation dialogs are host-only (the host decides). This gives viewers awareness without granting them veto power.

### Q6: Integration with git hooks?
Should the EBC engine integrate with git's own hook system (pre-commit, pre-push) in addition to Agent Hand hooks? **Recommendation**: Yes, in Phase 4+. The EBC engine can register git hooks that feed events into the classification pipeline. This provides an additional layer of protection for git operations specifically.

### Q7: How to handle "user always approves" patterns?
If a user approves every HIGH-risk action without reading the evidence, should the system detect this and warn? **Recommendation**: Track approval latency. If a user consistently approves in < 2 seconds, show a gentle reminder: "You've quickly approved 5 high-risk actions. Consider reviewing evidence more carefully." This is advisory only, never blocking.
