# Memory & Relationship System — SPEC

## 1. Overview

The Memory & Relationship System is the semantic backbone of Agent Hand. It manages cross-session state persistence, relationship-aware context bridging, and semantic abstraction layers that transform raw terminal output into structured insights.

This is the "connective tissue" between all modules. Where the ECS provides the runtime architecture and the Canvas provides visualization, the Memory System provides **meaning** — understanding what sessions are doing, how they relate, and when concurrent work is semantically converging or diverging.

### The "Sand in the Air" Metaphor

When multiple AI agents work concurrently on related codebases, their activities create semantic "particles" — file changes, function signatures, API calls, error patterns. Most of the time these particles are independent. But sometimes two agents are unknowingly converging on the same problem, or one agent is about to break something another agent depends on.

The Memory System detects this convergence: it captures semantic signals from each session, computes similarity, and alerts when "sand grains" in different sessions are close enough to collide. This transforms Agent Hand from a passive session manager into an active semantic coordinator.

### Key Design Principles

- **Hierarchical refinement**: raw context → relationships → abstractions → insights
- **Agent-driven discovery**: LLM semantic analysis discovers relationships humans miss
- **Cross-cutting integration**: Every module feeds into and reads from the memory layer
- **Efficient persistence**: Only persist what can't be reconstructed; summarize aggressively
- **Privacy-aware**: Users control what context crosses session boundaries

---

## 2. User Stories

### US-01: Cross-Session Context Bridge
> As a developer with "auth-service" and "api-gateway" sessions, I want the api-gateway session to know that auth-service just implemented JWT tokens, so the gateway can use the correct token format without me manually relaying the information.

**Acceptance**: When a relationship exists between sessions, context snapshots from one session are available as structured summaries in related sessions. A "context bridge" command in the target session retrieves relevant context.

### US-02: Semantic Collision Detection
> As a developer with 5 concurrent agents, I want to be alerted when two agents are modifying the same files or working on overlapping functionality, before they create merge conflicts.

**Acceptance**: System monitors file paths and function names mentioned in terminal output. When overlap exceeds threshold, a notification fires with details of the collision.

### US-03: Relationship Discovery
> As a developer who just created 4 new sessions for different parts of a project, I want the system to suggest relationships between them based on their working directories, file overlap, and API usage patterns.

**Acceptance**: After sessions run for 5+ minutes, the system analyzes their context and suggests relationships with confidence scores. User can accept/reject/modify suggestions.

### US-04: Insight Extraction
> As a developer returning to a project after a week, I want to see a summary of what all sessions accomplished, what decisions were made, and what's still in progress — without reading through terminal histories.

**Acceptance**: "Project summary" view aggregates insights across all sessions in a group, showing: completed work, pending decisions, blockers, and session interdependencies.

### US-05: Semantic Search Across Sessions
> As a developer, I want to search for "authentication" and find all sessions that dealt with auth-related work, even if they never used that exact word — they might have worked on "JWT", "tokens", "login", or "OAuth".

**Acceptance**: Semantic search powered by AI embeddings or keyword expansion. Results ranked by relevance with session context snippets.

### US-06: Memory Persistence Across Restarts
> As a developer, I want the relationships, insights, and context bridges to survive application restarts and even session deletion — the knowledge about a completed session is valuable even after the session is gone.

**Acceptance**: Memory layer persists independently of session lifecycle. Archived sessions retain their context summaries and relationship metadata.

---

## 3. Architecture

### 3.1 Layered Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Layer 4: Insights                                        │
│   Project summaries, decision logs, pattern detection    │
├─────────────────────────────────────────────────────────┤
│ Layer 3: Relationships                                   │
│   Cross-session connections, collision detection,        │
│   dependency tracking, semantic proximity                │
├─────────────────────────────────────────────────────────┤
│ Layer 2: Abstractions                                    │
│   AI summaries, entity extraction, topic classification  │
├─────────────────────────────────────────────────────────┤
│ Layer 1: Raw Context                                     │
│   Terminal captures, hook events, file paths, timestamps │
└─────────────────────────────────────────────────────────┘
```

Each layer refines the one below it. Raw context is voluminous and ephemeral. Insights are compact and persistent.

### 3.2 Core Data Structures

```rust
/// The memory store — persistent semantic knowledge
pub struct MemoryStore {
    /// Per-session context windows (rolling, bounded)
    pub session_contexts: HashMap<String, SessionContext>,

    /// Cross-session relationships with semantic metadata
    pub relationship_memory: Vec<RelationshipMemory>,

    /// Extracted entities (files, functions, APIs, concepts)
    pub entity_graph: EntityGraph,

    /// High-level insights (summaries, decisions, patterns)
    pub insights: Vec<Insight>,

    /// Archived sessions (context preserved after session deletion)
    pub archives: HashMap<String, ArchivedSession>,

    /// Collision detection state
    pub collision_tracker: CollisionTracker,
}

/// Layer 1: Raw context for a single session
pub struct SessionContext {
    pub session_id: String,
    pub title: String,
    pub group_path: String,
    pub project_path: PathBuf,

    /// Rolling window of context snapshots (bounded, oldest evicted)
    pub snapshots: VecDeque<ContextSnapshot>,
    pub max_snapshots: usize,              // Default: 100

    /// Extracted semantic signals from recent activity
    pub signals: Vec<SemanticSignal>,

    /// Current working set: files being modified
    pub working_files: HashSet<PathBuf>,

    /// Topics/concepts this session is working on
    pub topics: Vec<Topic>,

    /// Last AI analysis timestamp
    pub last_analyzed_at: Option<DateTime<Utc>>,
}

/// Layer 2: Semantic signal extracted from raw context
pub struct SemanticSignal {
    pub id: String,
    pub session_id: String,
    pub signal_type: SignalType,
    pub content: String,                   // The extracted data
    pub confidence: f64,                   // 0.0 - 1.0
    pub extracted_at: DateTime<Utc>,
    pub source_snapshot_id: Option<String>,
}

pub enum SignalType {
    FileModified(PathBuf),                 // File path detected in output
    FunctionDefined(String),               // Function/method name
    ApiEndpoint(String),                   // API route or URL
    ErrorPattern(String),                  // Recurring error signature
    DependencyUsed(String),                // Library/crate import
    ConceptMention(String),                // Semantic concept (auth, database, UI)
    ToolUsage(String),                     // Tool name + action
    DecisionMade(String),                  // Architectural decision
}

/// Layer 2: Topic classification
pub struct Topic {
    pub name: String,                      // e.g., "authentication", "database-schema"
    pub confidence: f64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub signal_count: usize,               // How many signals relate to this topic
}

/// Layer 3: Relationship with semantic enrichment
pub struct RelationshipMemory {
    pub relationship_id: String,           // Links to Relationship entity
    pub session_a_id: String,
    pub session_b_id: String,

    /// Semantic similarity score (0.0 - 1.0)
    pub semantic_similarity: f64,

    /// Shared entities (files, functions, concepts both sessions touch)
    pub shared_entities: Vec<SharedEntity>,

    /// Context bridge: summarized context from A relevant to B and vice versa
    pub bridge_a_to_b: Option<ContextBridge>,
    pub bridge_b_to_a: Option<ContextBridge>,

    /// Last analysis timestamp
    pub analyzed_at: DateTime<Utc>,

    /// Discovery method
    pub discovery: DiscoveryMethod,
}

pub struct SharedEntity {
    pub entity_type: SharedEntityType,
    pub name: String,
    pub session_a_signals: Vec<String>,    // Signal IDs from session A
    pub session_b_signals: Vec<String>,    // Signal IDs from session B
}

pub enum SharedEntityType {
    File,
    Function,
    ApiEndpoint,
    Concept,
    Dependency,
}

pub enum DiscoveryMethod {
    UserCreated,                           // User manually created relationship
    AiDiscovered { prompt: String },       // LLM found the relationship
    FileOverlap,                           // Detected via shared file paths
    ParentChild,                           // Inherited from fork
}

/// Layer 3: Context bridge — relevant context from one session for another
pub struct ContextBridge {
    pub source_session_id: String,
    pub target_session_id: String,

    /// AI-generated summary of what's relevant from source for target
    pub summary: String,

    /// Specific items from source that target should know about
    pub relevant_items: Vec<BridgeItem>,

    pub generated_at: DateTime<Utc>,
    pub stale: bool,
}

pub struct BridgeItem {
    pub item_type: BridgeItemType,
    pub description: String,
    pub source_signal_id: String,
}

pub enum BridgeItemType {
    ApiContract,                           // "auth-service exposes POST /api/auth/token"
    DataSchema,                            // "User table has columns: id, email, role"
    Decision,                              // "Decided to use RS256 for JWT signing"
    Warning,                               // "auth-service had timeout issues with DB"
    Dependency,                            // "auth-service uses jsonwebtoken 9.3"
}

/// Layer 4: High-level insight
pub struct Insight {
    pub id: String,
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub related_sessions: Vec<String>,     // Session IDs involved
    pub related_relationships: Vec<String>,
    pub generated_at: DateTime<Utc>,
    pub confidence: f64,
    pub status: InsightStatus,
}

pub enum InsightType {
    ProjectSummary,                        // Aggregated progress across sessions
    DecisionLog,                           // Architectural decisions made
    PatternDetected,                       // Recurring behavior across sessions
    CollisionWarning,                      // Two sessions converging on same area
    ProgressMilestone,                     // Significant completion event
    BlockerIdentified,                     // Something blocking multiple sessions
}

pub enum InsightStatus {
    Active,                                // Currently relevant
    Resolved,                              // No longer relevant (e.g., blocker fixed)
    Archived,                              // Historically interesting
}
```

### 3.3 Entity Graph

The entity graph tracks semantic entities across all sessions — shared files, functions, APIs, and concepts form a graph that reveals hidden connections.

```rust
pub struct EntityGraph {
    pub nodes: HashMap<String, SemanticEntity>,
    pub edges: Vec<EntityEdge>,
}

pub struct SemanticEntity {
    pub id: String,
    pub entity_type: SemanticEntityType,
    pub name: String,
    pub sessions: HashSet<String>,         // Which sessions reference this entity
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub mention_count: usize,
}

pub enum SemanticEntityType {
    File,
    Function,
    Module,
    ApiEndpoint,
    Database,
    Concept,
    Error,
}

pub struct EntityEdge {
    pub source: String,                    // Entity ID
    pub target: String,                    // Entity ID
    pub edge_type: EntityEdgeType,
}

pub enum EntityEdgeType {
    Contains,                              // Module contains function
    Calls,                                 // Function calls function
    Reads,                                 // Function reads from database
    Writes,                                // Function writes to database
    DependsOn,                             // Module depends on module
}
```

### 3.4 Collision Detection

```rust
pub struct CollisionTracker {
    /// File paths being modified by each session
    pub file_watchers: HashMap<PathBuf, HashSet<String>>,  // path → session_ids

    /// Function names being modified by each session
    pub function_watchers: HashMap<String, HashSet<String>>,

    /// Active collision warnings
    pub active_collisions: Vec<Collision>,

    /// Sensitivity threshold (0.0 - 1.0)
    pub sensitivity: f64,
}

pub struct Collision {
    pub id: String,
    pub collision_type: CollisionType,
    pub sessions: Vec<String>,             // Session IDs involved
    pub subject: String,                   // What's colliding (file path, function name)
    pub severity: CollisionSeverity,
    pub detected_at: DateTime<Utc>,
    pub resolved: bool,
}

pub enum CollisionType {
    FileConflict,                          // Multiple sessions modifying same file
    FunctionOverlap,                       // Multiple sessions defining same function
    ApiConflict,                           // Conflicting API contract definitions
    ConceptDivergence,                     // Same concept, different implementations
}

pub enum CollisionSeverity {
    Low,                                   // Awareness — might be intentional
    Medium,                                // Attention — likely unintentional overlap
    High,                                  // Action needed — will cause merge conflict
}
```

### 3.5 AI Analysis Pipeline

The memory system uses AI to transform raw context into structured semantic data. This runs as an ECS system (see Spec 05) or as a standalone background task.

```
Raw Context (Layer 1)
    │
    ▼
┌─────────────────────────────┐
│ Signal Extraction (Layer 2) │  ← AI prompt: "Extract files, functions, APIs,
│                             │     errors, and decisions from this terminal output"
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│ Topic Classification        │  ← AI prompt: "What topics is this session
│                             │     working on? Classify: auth, db, ui, api, etc."
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│ Relationship Analysis       │  ← AI prompt: "Given sessions A and B with these
│ (Layer 3)                   │     signals, what's their semantic relationship?
│                             │     What context from A is relevant to B?"
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│ Insight Generation          │  ← AI prompt: "Given all sessions in group X,
│ (Layer 4)                   │     summarize progress, decisions, and blockers"
└─────────────────────────────┘
```

**Analysis triggers**:
- Session status change (Running → Idle: session did something worth analyzing)
- Periodic timer (every 5 minutes for active sessions)
- User request (manual refresh)
- Collision detection signal (new file overlap detected)

**AI prompt templates**:

```
# Signal Extraction
System: You are analyzing terminal output from an AI coding agent session.
Extract structured signals from the output.

Session: {title} ({tool})
Working directory: {project_path}
Terminal output (last {n} lines):
{pane_content}

Extract as JSON:
- files_modified: [list of file paths mentioned as created/modified]
- functions_defined: [function/method names defined or modified]
- api_endpoints: [API routes or URLs referenced]
- errors: [error messages or patterns]
- dependencies: [libraries/crates imported or used]
- decisions: [architectural or implementation decisions stated]
- concepts: [high-level concepts being worked on]

# Context Bridge Generation
System: You are creating a context bridge between two AI coding agent sessions.
Summarize what's relevant from Session A for Session B.

Session A ({a_title}):
Topics: {a_topics}
Recent signals: {a_signals}

Session B ({b_title}):
Topics: {b_topics}
Recent signals: {b_signals}

Relationship type: {relation_type}

Generate:
1. A 2-3 sentence summary of what B needs to know from A
2. Specific items (API contracts, data schemas, decisions, warnings)
3. Any potential conflicts between A and B's approaches

# Relationship Discovery
System: Analyze these sessions and suggest relationships.

Sessions:
{for each session: title, topics, files, recent_signals}

Suggest relationships as JSON:
[{
  "session_a": "id",
  "session_b": "id",
  "type": "Peer|Dependency|Collaboration",
  "reason": "why these are related",
  "confidence": 0.0-1.0,
  "shared_entities": ["entity names"]
}]
```

### 3.6 Persistence Architecture

```
~/.agent-hand/profiles/{profile}/
├── sessions.json              # Existing: instances, groups, relationships
├── events.jsonl               # ECS event log (Spec 05)
├── memory/
│   ├── contexts/
│   │   ├── {session_id}.json  # Per-session context (Layer 1-2)
│   │   └── ...
│   ├── relationships/
│   │   ├── {relationship_id}.json  # Relationship memory (Layer 3)
│   │   └── ...
│   ├── entity_graph.json      # Semantic entity graph
│   ├── insights.json          # Active insights (Layer 4)
│   ├── collisions.json        # Active collision warnings
│   └── archives/
│       ├── {session_id}.json  # Archived session context
│       └── ...
```

**Persistence strategy**:
- **Layer 1 (raw context)**: Not persisted — reconstructable from terminal. Only current snapshots kept in memory.
- **Layer 2 (signals, topics)**: Persisted per-session as JSON. Bounded: max 500 signals, oldest evicted.
- **Layer 3 (relationships, bridges)**: Persisted per-relationship. Updated on relationship analysis.
- **Layer 4 (insights)**: Fully persisted. Manual cleanup by user.
- **Archives**: Created on session deletion. Contains final signals, topics, and relationship contributions.

---

## 4. Protocol / API

### 4.1 Context Bridge Protocol

When a user wants to pull context from a related session:

```rust
/// Request context from related sessions
pub struct ContextBridgeRequest {
    pub target_session_id: String,         // The session requesting context
    pub source_filter: Option<Vec<String>>, // Specific source sessions (None = all related)
    pub item_types: Option<Vec<BridgeItemType>>, // Filter by item type
    pub max_items: usize,                  // Max items to return (default: 10)
}

pub struct ContextBridgeResponse {
    pub target_session_id: String,
    pub bridges: Vec<ContextBridge>,
    pub collisions: Vec<Collision>,        // Any active collisions involving target
}
```

**CLI**:
```bash
# Get context bridge for a session
agent-hand context auth-service

# Output:
# === Context Bridge: auth-service ===
#
# From: api-gateway (dependency)
#   API Contract: Expects POST /api/auth/token returning { token: string, expires: number }
#   Warning: api-gateway uses axios timeout of 5s; auth-service P99 is 3.2s
#
# From: frontend (peer)
#   Decision: Frontend uses httpOnly cookies for token storage
#   Dependency: Expects refresh token endpoint at POST /api/auth/refresh
#
# Active Collisions: None
```

### 4.2 Collision Alert Protocol

```rust
/// Collision alert emitted as ECS event
pub struct CollisionDetected {
    pub collision: Collision,
}

/// Collision resolved (manual or automatic)
pub struct CollisionResolved {
    pub collision_id: String,
    pub resolution: CollisionResolution,
}

pub enum CollisionResolution {
    ManualDismiss,                         // User acknowledged and dismissed
    FilesDiverged,                         // Sessions no longer touching same files
    SessionEnded,                          // One of the sessions ended
    MergeCompleted,                        // Sessions merged their changes
}
```

### 4.3 Relationship Discovery Protocol

```rust
/// Suggested relationship from AI analysis
pub struct RelationshipSuggestion {
    pub session_a_id: String,
    pub session_b_id: String,
    pub suggested_type: RelationType,
    pub reason: String,
    pub confidence: f64,
    pub shared_entities: Vec<String>,
    pub suggested_at: DateTime<Utc>,
}

/// User response to suggestion
pub enum SuggestionResponse {
    Accept,                                // Create the relationship
    AcceptWithModification {               // Create with different type/label
        relation_type: RelationType,
        label: Option<String>,
    },
    Reject,                                // Don't create; suppress future similar suggestions
    Defer,                                 // Not now; may suggest again later
}
```

### 4.4 Memory Query API

```rust
pub enum MemoryQuery {
    /// Search across all sessions by semantic content
    SemanticSearch {
        query: String,
        session_filter: Option<Vec<String>>,
        signal_types: Option<Vec<SignalType>>,
        max_results: usize,
    },

    /// Get project-level summary
    ProjectSummary {
        group_path: String,
        include_archived: bool,
    },

    /// Get relationship graph for visualization
    RelationshipGraph {
        center_session: Option<String>,    // None = full graph
        depth: usize,                      // How many hops from center
    },

    /// Get collision status
    CollisionStatus {
        session_filter: Option<Vec<String>>,
    },

    /// Get entity graph
    EntityGraph {
        entity_types: Option<Vec<SemanticEntityType>>,
        session_filter: Option<Vec<String>>,
    },
}
```

---

## 5. UI/UX Design

### 5.1 Context Bridge Panel

Accessible from session detail view via `b` (bridge):

```
╭─ Context Bridge ─ auth-service ────────────────────────────────╮
│                                                                │
│  ─── From: api-gateway (dependency) ───                       │
│                                                                │
│  API Contract:                                                 │
│    POST /api/auth/token → { token: string, expires: number }  │
│    POST /api/auth/refresh → { token: string }                 │
│                                                                │
│  Warning:                                                      │
│    api-gateway axios timeout: 5s (auth P99: 3.2s)            │
│                                                                │
│  ─── From: frontend (peer) ───                                │
│                                                                │
│  Decision:                                                     │
│    Using httpOnly cookies for token storage (not localStorage)│
│                                                                │
│  Dependency:                                                   │
│    Expects refresh token rotation on every use                │
│                                                                │
│  ─── Collisions ───                                           │
│  ⚠ MEDIUM: src/types/auth.ts modified by both auth-service   │
│    and api-gateway                                             │
│                                                                │
│  [r]Refresh  [c]Copy to clipboard  [q]Back                    │
╰────────────────────────────────────────────────────────────────╯
```

### 5.2 Collision Alert (Toast)

When a collision is detected, a toast notification appears:

```
╭─ Dashboard ──────────────────────────────────────────╮
│                                                      │
│  ... normal dashboard content ...                    │
│                                                      │
│  ┌─ ⚠ Collision Detected ────────────────────────┐  │
│  │ auth-service & api-gateway both modifying     │  │
│  │ src/types/auth.ts                             │  │
│  │ [d]Dismiss  [v]View details  [r]Resolve       │  │
│  └───────────────────────────────────────────────┘  │
╰──────────────────────────────────────────────────────╯
```

### 5.3 Relationship Discovery Suggestions

When AI discovers potential relationships, shown as a non-intrusive banner:

```
╭─ Dashboard ──────────────────────────────────────────╮
│  ┌─ 💡 Suggested Relationship ───────────────────┐  │
│  │ auth-service ═══ api-gateway (Peer)           │  │
│  │ Reason: Both modify src/types/ and share      │  │
│  │ jsonwebtoken dependency (confidence: 0.85)     │  │
│  │ [y]Accept  [n]Reject  [m]Modify  [l]Later     │  │
│  └───────────────────────────────────────────────┘  │
│                                                      │
│  ... normal dashboard content ...                    │
╰──────────────────────────────────────────────────────╯
```

### 5.4 Project Summary View

Accessible via `S` (shift+s) from group-level selection:

```
╭─ Project Summary ─ work/backend ───────────────────────────────╮
│                                                                │
│  Period: Last 7 days (Mar 1-6, 2026)                          │
│  Sessions: 4 active, 2 archived                                │
│                                                                │
│  ─── Progress ───                                             │
│  ✓ JWT authentication middleware (auth-service)               │
│  ✓ Database schema v14 migration (db-migrations)              │
│  ◐ API gateway routing (api-gateway) — 70% estimated         │
│  ○ Frontend auth flow (frontend) — not started                │
│                                                                │
│  ─── Decisions Made ───                                       │
│  • RS256 for JWT signing (auth-service, Mar 3)                │
│  • httpOnly cookies over localStorage (frontend, Mar 4)       │
│  • Rate limiting at gateway level (api-gateway, Mar 5)        │
│                                                                │
│  ─── Active Blockers ───                                      │
│  ⚠ api-gateway blocked on auth-service token format           │
│  ⚠ frontend waiting for refresh token endpoint spec           │
│                                                                │
│  ─── Relationships ───                                        │
│  auth-service ──▶ api-gateway (dependency)                    │
│  auth-service ═══ frontend (peer)                             │
│  db-migrations ──▶ auth-service (dependency)                  │
│                                                                │
│  [r]Refresh  [e]Export  [q]Back                                │
╰────────────────────────────────────────────────────────────────╯
```

### 5.5 Semantic Search Results

Accessible via `?` (semantic search):

```
╭─ Semantic Search: "authentication" ────────────────────────────╮
│                                                                │
│  4 results across 3 sessions                                   │
│                                                                │
│  1. auth-service (relevance: 0.95)                            │
│     Topics: JWT, middleware, token-validation                  │
│     "Implementing JWT token validation middleware with RS256"  │
│                                                                │
│  2. api-gateway (relevance: 0.78)                             │
│     Topics: routing, auth-headers, token-forwarding            │
│     "Adding Authorization header extraction and forwarding"   │
│                                                                │
│  3. frontend (relevance: 0.62)                                │
│     Topics: login-form, cookies, session-management            │
│     "Planning auth flow: login form → API call → cookie set" │
│                                                                │
│  4. [archived] old-auth (relevance: 0.45)                     │
│     Topics: basic-auth, deprecated                             │
│     "Basic auth implementation (deprecated, replaced by JWT)" │
│                                                                │
│  [Enter]View session  [b]Bridge  [/]Refine  [q]Back           │
╰────────────────────────────────────────────────────────────────╯
```

---

## 6. Implementation Strategy

### Phase 1: Signal Extraction Pipeline (Week 1-2)
- Implement `SessionContext` and `SemanticSignal` data structures
- Build signal extraction from pane captures (regex-based, no AI yet):
  - File paths: detect `src/`, `./`, absolute paths in terminal output
  - Function names: detect `fn `, `function `, `def `, `class ` patterns
  - Error patterns: detect `error`, `Error`, `FAILED`, `panic`
- Store signals in per-session context files
- Basic collision detection: file path overlap across sessions

### Phase 2: AI-Powered Analysis (Week 3-4)
- Integrate with existing `Summarizer` for signal extraction via AI
- Implement topic classification (AI-powered)
- Build context bridge generation (AI-powered)
- Rate limiting: max 1 analysis per session per 5 minutes
- Cache management: evict old signals, keep topic summaries

### Phase 3: Relationship Discovery (Week 5-6)
- Implement `RelationshipMemory` enrichment for existing relationships
- Build AI-powered relationship suggestion pipeline
- Suggestion UI: banner + accept/reject/modify workflow
- Collision detection upgrade: function-level and concept-level overlap
- Collision alert UI: toast notifications

### Phase 4: Insights and Summaries (Week 7-8)
- Implement `Insight` generation from aggregated signals
- Build project summary view (group-level)
- Build decision log extraction
- Implement blocker identification across sessions
- Archive system: preserve context when sessions are deleted

### Phase 5: Search and Integration (Week 9-10)
- Semantic search across sessions (keyword expansion via AI)
- Entity graph visualization (feeds into Canvas, Spec 04)
- Integration with ECS (Spec 05): memory analysis as a System
- Integration with collaboration (Spec 01): shared memory across viewers
- CLI commands for memory queries

---

## 7. Dependencies

### Depends On
- **Session system** (`src/session/`): Source of session data and pane captures
- **Relationship system** (`src/session/relationships.rs`): Relationships to enrich with memory
- **AI Summarizer** (`src/ai/summarize.rs`): AI-powered analysis pipeline
- **Context Snapshots** (`src/session/context.rs`): Existing snapshot infrastructure
- **Storage** (`src/session/storage.rs`): Persistence infrastructure
- **Hook system** (`src/hooks/`): Tool usage events for signal extraction

### Enables
- **Canvas View** (Spec 04): Entity graph provides semantic edges for canvas visualization
- **ECS Runtime** (Spec 05): Memory analysis becomes `MemorySystem`; signals become components
- **Collaboration** (Spec 01): Context bridges shared with remote viewers
- **Fork System** (Spec 02): Fork inherits parent's context and relationships
- **Presence Tracking** (Spec 03): Viewer focus tracked as semantic signal
- **Security** (Spec 06): Privacy controls on what context crosses session boundaries

### New Dependencies (crates)
- None strictly required. AI analysis uses existing `ai_api_provider` integration.
- Consider `tantivy` (Rust full-text search) for semantic search if performance warrants. For initial implementation, regex + AI keyword expansion suffices.

---

## 8. Open Questions

1. **Privacy boundaries**: Should context bridges be opt-in per session, per relationship, or global? Proposed: opt-in per relationship, with a global default in config. Sensitive sessions (e.g., working with credentials) should be excluded by default.

2. **AI cost management**: Signal extraction and relationship analysis consume AI tokens. How to budget? Proposed: configurable analysis frequency (default: on status change to Idle, max once per 5 minutes). Users can disable AI analysis entirely with `ai.memory_analysis = false`.

3. **Signal accuracy**: Regex-based signal extraction (Phase 1) will have false positives. How to validate? Proposed: signals have confidence scores. Regex extraction gets 0.5-0.7 confidence. AI extraction gets 0.7-0.9. Only signals above threshold (configurable, default 0.5) are used for collision detection.

4. **Archive retention**: How long to keep archived session context? Proposed: configurable, default 30 days. Archives auto-deleted after retention period. Users can mark archives as "permanent."

5. **Entity graph scale**: For projects with hundreds of files, the entity graph could grow large. How to bound it? Proposed: entity graph is scoped to active sessions + recent archives. Entities not referenced in 7 days are pruned.

6. **Collision sensitivity**: Too sensitive → alert fatigue. Too lenient → miss real conflicts. Proposed: three sensitivity presets (Low/Medium/High) configurable per group. Medium is default: alerts on same-file modification, not on same-directory.

7. **Cross-project relationships**: Should the memory system track relationships across different `group_path` roots? Proposed: yes, but with lower priority for analysis. Same-group relationships analyzed first.

8. **Embedding model**: For true semantic search (beyond keyword expansion), we'd need an embedding model. Should we support local embeddings (e.g., via `rust-bert`) or API-based only? Proposed: API-based initially (using connected AI provider). Local embeddings as future optimization for privacy-sensitive deployments.

9. **Real-time vs batch**: Should signal extraction happen in real-time (on every pane capture) or in batch (on status change)? Proposed: batch on status change. Real-time is too expensive and noisy — terminal output during active work is full of intermediate states that don't represent meaningful signals.

10. **Integration with agent memory**: AI agents (Claude Code, etc.) have their own memory/context systems. Should Agent Hand's memory system interact with them? Proposed: not initially. Agent Hand's memory is orthogonal — it's the user's cross-agent memory, not any single agent's memory. Future: export context bridges as agent-readable files in working directories.
