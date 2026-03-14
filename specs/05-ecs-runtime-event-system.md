# ECS Runtime & Event System — SPEC

## 1. Overview

The ECS (Entity-Component-System) Runtime replaces the current direct-struct architecture with a reactive game-world model. Every session, relationship, user, and collaboration becomes an **entity**. State, metrics, and configuration become **components** attached to entities. Scheduling, analytics, AI processing, and sound effects become **systems** that process component data each tick.

This transforms Agent Hand from a "session list manager" into a living, reactive system where state changes propagate through event buses, analytics accumulate passively, and external agents can create/destroy/connect sessions programmatically.

### Motivation

The current `App` struct contains 150+ fields mixing UI state, session data, networking, analytics, and configuration. This creates:
- **Tight coupling**: Adding a feature (e.g., analytics on relationship usage) requires modifying App
- **No event history**: Status changes are point-in-time; there's no record of transitions
- **Limited extensibility**: Sound effects are hardcoded; new state-change responses require code changes
- **No scheduling**: External agents can't programmatically manage sessions

ECS solves this by decomposing the monolith into composable entities + components processed by independent systems.

### Key Design Principles

- **Lightweight ECS**: Not Bevy — a minimal, focused ECS tailored to session management
- **Event-sourced**: All state changes produce events; systems react to events
- **Observable**: External agents and plugins can subscribe to entity changes
- **Incremental adoption**: Can coexist with current App struct during migration
- **Game-loop tick**: Systems run each frame (16ms for 60fps TUI), enabling animations and scheduling

---

## 2. User Stories

### US-01: Event Timeline
> As a developer, I want to see a timeline of everything that happened across my sessions — status changes, tool usage, errors — so I can reconstruct what went wrong when a session fails.

**Acceptance**: Event log viewable per-session and globally. Events include: status transitions, hook events (tool failures, permission requests), relationship changes, context snapshots.

### US-02: Operation Analytics
> As a developer managing 10+ concurrent sessions, I want to see which sessions are most active, which are blocked the longest, and what tools are being used most, so I can optimize my workflow.

**Acceptance**: Analytics dashboard showing: session activity heatmap, average time-to-resolution, tool usage distribution, relationship utilization.

### US-03: Agent-Driven Session Creation
> As an AI orchestrator agent, I want to programmatically create sessions, set up relationships between them, and schedule them to start in dependency order, so I can decompose a large task automatically.

**Acceptance**: CLI commands or API that creates entities, attaches components, and establishes relationships. Sessions start in topological order of dependencies.

### US-04: Generalized State-Change Responses
> As a developer, I want to configure custom responses to state changes beyond just sound effects — e.g., send a desktop notification when any session enters Error state, or auto-summarize when a session goes Idle.

**Acceptance**: Configurable reaction rules: `when {condition} then {action}`. Conditions: status change, tool event, relationship event. Actions: sound, notification, summarize, log, webhook.

### US-05: Plugin System
> As a power user, I want to write custom systems (plugins) that process session events — e.g., a system that tracks how long each session spends in each status and generates daily reports.

**Acceptance**: Systems defined as Rust traits with `run()` method. Plugin systems loaded from config, receive same event stream as built-in systems.

### US-06: Session Scheduling
> As a developer, I want to define that "session B should start after session A finishes" and have the system automatically manage the lifecycle.

**Acceptance**: Dependency relationships with auto-start behavior. When depended-upon session reaches target status, dependent session starts automatically.

---

## 3. Architecture

### 3.1 ECS Crate Selection: Custom Lightweight

**Decision**: Build a custom lightweight ECS rather than use bevy_ecs or hecs.

**Rationale**:
- bevy_ecs: Too heavy — pulls in Bevy's scheduler, parallelism, and change detection systems designed for game rendering. We have ~100 entities, not millions.
- hecs: Closer but missing event system, scheduling, and query caching.
- Custom: ~500 lines of code. Entities are string-keyed (session IDs already exist). Components are trait objects. Systems are trait implementations.

The entity count in Agent Hand is small (tens to low hundreds), so the ECS's value isn't in cache-friendly iteration — it's in the **architectural decomposition** and **event-driven reactivity**.

### 3.2 Core Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Game Loop (tick)                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Input    │→ │ Systems  │→ │ Render   │              │
│  │ System   │  │ Pipeline │  │ System   │              │
│  └──────────┘  └──────────┘  └──────────┘              │
│       │              │              │                    │
│       ▼              ▼              ▼                    │
│  ┌──────────────────────────────────────────────────┐   │
│  │              World (Entity Storage)               │   │
│  │  ┌────────┐  ┌────────┐  ┌────────┐             │   │
│  │  │Entity A│  │Entity B│  │Entity C│  ...         │   │
│  │  │[comp1] │  │[comp1] │  │[comp2] │             │   │
│  │  │[comp2] │  │[comp3] │  │[comp4] │             │   │
│  │  └────────┘  └────────┘  └────────┘             │   │
│  └──────────────────────────────────────────────────┘   │
│       ▲              │                                   │
│       │              ▼                                   │
│  ┌──────────────────────────────────────────────────┐   │
│  │              Event Bus                            │   │
│  │  [StatusChanged] [HookReceived] [RelationAdded]  │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 3.3 World

```rust
/// The ECS world — holds all entities, components, and event queues
pub struct World {
    entities: HashMap<EntityId, Entity>,
    component_stores: HashMap<TypeId, Box<dyn AnyComponentStore>>,
    event_bus: EventBus,
    resources: HashMap<TypeId, Box<dyn Any>>,  // Singleton resources (config, tmux manager, etc.)
    next_entity_id: u64,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct EntityId(u64);

pub struct Entity {
    pub id: EntityId,
    pub entity_type: EntityType,
    pub name: String,                    // Human-readable (session title, group path, etc.)
    pub alive: bool,
    pub created_at: DateTime<Utc>,
}

pub enum EntityType {
    Session,
    Group,
    Relationship,
    User,                                // For collaboration
    Viewer,                              // Remote viewer
}
```

### 3.4 Components

Components are plain data structs. Each component type has its own storage.

```rust
/// Trait for all components
pub trait Component: 'static + Send + Sync {}

// ─── Session Components ───

/// Core session identity (replaces Instance fields)
pub struct SessionIdentity {
    pub title: String,
    pub project_path: PathBuf,
    pub group_path: String,
    pub tool: Tool,
    pub command: String,
    pub label: String,
    pub label_color: LabelColor,
}

/// Session runtime state
pub struct SessionState {
    pub status: Status,
    pub tmux_session_name: String,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub last_running_at: Option<DateTime<Utc>>,
    pub last_waiting_at: Option<DateTime<Utc>>,
    pub ptmx_count: u32,
}

/// AI provider binding
pub struct AiBinding {
    pub claude_session_id: Option<String>,
    pub claude_detected_at: Option<DateTime<Utc>>,
    pub gemini_session_id: Option<String>,
    pub gemini_detected_at: Option<DateTime<Utc>>,
}

/// Sharing state (pro)
pub struct SharingComponent {
    pub state: SharingState,
}

/// Accumulated metrics for a session
pub struct SessionMetrics {
    pub total_running_time: Duration,
    pub total_waiting_time: Duration,
    pub total_idle_time: Duration,
    pub status_transitions: Vec<StatusTransition>,
    pub tool_events: Vec<ToolEvent>,
    pub error_count: u32,
    pub created_at: DateTime<Utc>,
}

pub struct StatusTransition {
    pub from: Status,
    pub to: Status,
    pub at: DateTime<Utc>,
}

pub struct ToolEvent {
    pub tool_name: String,
    pub event_type: ToolEventType,       // Success, Failure, PermissionRequest
    pub at: DateTime<Utc>,
}

/// AI-generated preview (for canvas)
pub struct PreviewComponent {
    pub summary: String,
    pub generated_at: DateTime<Utc>,
    pub stale: bool,
}

// ─── Relationship Components ───

/// Relationship data (attached to relationship entities)
pub struct RelationshipData {
    pub relation_type: RelationType,
    pub session_a: EntityId,
    pub session_b: EntityId,
    pub label: Option<String>,
    pub bidirectional: bool,
    pub metadata: HashMap<String, String>,
}

/// Scheduling constraint (attached to relationship entities)
pub struct ScheduleConstraint {
    pub trigger_status: Status,          // When source reaches this status...
    pub action: ScheduleAction,          // ...perform this action on target
}

pub enum ScheduleAction {
    Start,                               // Start the target session
    Notify,                              // Send notification
    Summarize,                           // Trigger AI summary
    Custom(String),                      // Named custom action
}

// ─── User Components ───

pub struct UserIdentity {
    pub display_name: String,
    pub viewer_id: Option<String>,
    pub permission: SharePermission,
}

pub struct UserPresence {
    pub connected: bool,
    pub focused_session: Option<EntityId>,
    pub latency_ms: u32,
    pub connected_at: DateTime<Utc>,
}

// ─── Group Components ───

pub struct GroupData {
    pub name: String,
    pub path: String,
    pub expanded: bool,
    pub order: i32,
}
```

### 3.5 Event Bus

```rust
pub struct EventBus {
    queues: HashMap<TypeId, Box<dyn AnyEventQueue>>,
    subscribers: HashMap<TypeId, Vec<Box<dyn Fn(&dyn Any)>>>,
}

/// Events are emitted by systems and consumed by other systems
pub trait Event: 'static + Send + Clone {}

// ─── Core Events ───

#[derive(Clone)]
pub struct StatusChanged {
    pub entity: EntityId,
    pub from: Status,
    pub to: Status,
    pub at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct HookEventReceived {
    pub entity: EntityId,              // Session entity that produced the hook
    pub kind: HookEventKind,
    pub at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct RelationshipCreated {
    pub relationship_entity: EntityId,
    pub session_a: EntityId,
    pub session_b: EntityId,
    pub relation_type: RelationType,
}

#[derive(Clone)]
pub struct RelationshipRemoved {
    pub relationship_entity: EntityId,
}

#[derive(Clone)]
pub struct EntitySpawned {
    pub entity: EntityId,
    pub entity_type: EntityType,
}

#[derive(Clone)]
pub struct EntityDespawned {
    pub entity: EntityId,
    pub entity_type: EntityType,
}

#[derive(Clone)]
pub struct SessionAttached {
    pub entity: EntityId,
    pub user: Option<EntityId>,
}

#[derive(Clone)]
pub struct SessionDetached {
    pub entity: EntityId,
    pub user: Option<EntityId>,
}

#[derive(Clone)]
pub struct ViewerJoined {
    pub session: EntityId,
    pub viewer: EntityId,
    pub permission: SharePermission,
}

#[derive(Clone)]
pub struct ViewerLeft {
    pub session: EntityId,
    pub viewer: EntityId,
}

#[derive(Clone)]
pub struct ContextSnapshotTaken {
    pub session: EntityId,
    pub snapshot: ContextSnapshot,
}

#[derive(Clone)]
pub struct PreviewUpdated {
    pub entity: EntityId,
    pub summary: String,
}
```

### 3.6 Systems

Systems are the behavior. Each system runs once per tick (or on specific schedules).

```rust
/// Trait all systems implement
pub trait System: Send + Sync {
    fn name(&self) -> &str;
    fn run(&mut self, world: &mut World, dt: Duration);
    fn priority(&self) -> SystemPriority { SystemPriority::Normal }
    fn schedule(&self) -> SystemSchedule { SystemSchedule::EveryTick }
}

pub enum SystemPriority {
    High,        // Input, status detection
    Normal,      // Analytics, previews
    Low,         // Cleanup, persistence
}

pub enum SystemSchedule {
    EveryTick,                    // 60fps — UI, input
    Interval(Duration),           // Periodic — status polling, persistence
    OnEvent(TypeId),              // Reactive — triggered by specific event
}
```

**Built-in Systems**:

```rust
// ─── High Priority ───

/// Polls tmux for status and emits StatusChanged events
pub struct StatusDetectionSystem {
    poll_interval: Duration,
    last_poll: HashMap<EntityId, Instant>,
}

/// Processes hook events from JSONL file
pub struct HookReceiverSystem {
    receiver: EventReceiver,
}

/// Processes keyboard/mouse input, updates focus/selection
pub struct InputSystem;

// ─── Normal Priority ───

/// Accumulates metrics from events
pub struct MetricsSystem;

/// Generates AI previews for sessions with stale previews
pub struct PreviewSystem {
    summarizer: Summarizer,
    rate_limiter: RateLimiter,
}

/// Responds to state changes with configured actions (sounds, notifications, etc.)
pub struct ReactionSystem {
    rules: Vec<ReactionRule>,
}

/// Manages session scheduling based on dependency relationships
pub struct SchedulerSystem;

/// Records events to persistent log
pub struct EventLogSystem {
    log_path: PathBuf,
    buffer: Vec<LogEntry>,
}

/// Manages collaboration/relay connections
pub struct CollaborationSystem;

// ─── Low Priority ───

/// Periodically saves world state to disk
pub struct PersistenceSystem {
    save_interval: Duration,
    last_save: Instant,
}

/// Cleans up dead entities (error sessions, expired shares)
pub struct CleanupSystem;

/// Renders the TUI frame from world state
pub struct RenderSystem;
```

### 3.7 Reaction System (Generalized Sound Effects)

The current sound effect system is a special case of "react to state change." The ReactionSystem generalizes this.

```rust
pub struct ReactionRule {
    pub name: String,
    pub condition: ReactionCondition,
    pub action: ReactionAction,
    pub cooldown: Option<Duration>,       // Prevent rapid re-triggering
    pub enabled: bool,
}

pub enum ReactionCondition {
    StatusChange { from: Option<Status>, to: Status },
    HookEvent { kind: HookEventKind },
    EntitySpawned { entity_type: EntityType },
    EntityDespawned { entity_type: EntityType },
    MetricThreshold { metric: String, threshold: f64 },
    Custom(String),                       // Named custom condition
}

pub enum ReactionAction {
    PlaySound { sound: String },          // Existing sound system
    DesktopNotification { title: String, body: String },
    AiSummarize { entity: EntitySelector },
    Log { message: String },
    Webhook { url: String, payload: String },
    StartSession { entity: EntitySelector },  // Auto-start dependent sessions
    Custom(String),                       // Named custom action
}

pub enum EntitySelector {
    This,                                 // The entity that triggered the condition
    Related { relation_type: RelationType },
    ById(EntityId),
    ByName(String),
}
```

**Configuration** (in `config.toml`):
```toml
[[reactions]]
name = "error-alert"
condition = { type = "status_change", to = "Error" }
action = { type = "desktop_notification", title = "Session Error", body = "{entity.name} entered Error state" }

[[reactions]]
name = "idle-summarize"
condition = { type = "status_change", to = "Idle" }
action = { type = "ai_summarize", entity = "this" }
cooldown = "60s"

[[reactions]]
name = "waiting-sound"
condition = { type = "status_change", to = "Waiting" }
action = { type = "play_sound", sound = "attention" }

[[reactions]]
name = "dep-auto-start"
condition = { type = "status_change", to = "Idle" }
action = { type = "start_session", entity = { type = "related", relation_type = "Dependency" } }
```

### 3.8 Game Loop

```rust
pub struct GameLoop {
    world: World,
    systems: Vec<Box<dyn System>>,
    tick_rate: Duration,                  // 16ms for 60fps
    running: bool,
}

impl GameLoop {
    pub async fn run(&mut self) {
        let mut last_tick = Instant::now();

        loop {
            let now = Instant::now();
            let dt = now - last_tick;
            last_tick = now;

            // 1. Drain external inputs (terminal events, hook events)
            // 2. Run systems in priority order
            for system in &mut self.systems {
                if system.should_run(now) {
                    system.run(&mut self.world, dt);
                }
            }

            // 3. Flush event queues (events from this tick available to next tick)
            self.world.event_bus.flush();

            // 4. Sleep until next tick
            let elapsed = now.elapsed();
            if elapsed < self.tick_rate {
                tokio::time::sleep(self.tick_rate - elapsed).await;
            }
        }
    }
}
```

---

## 4. Protocol / API

### 4.1 Agent Scheduling API

External agents (or CLI commands) can create and manage entities.

```rust
/// CLI commands for agent-driven session management
pub enum AgentCommand {
    /// Create a session entity with components
    CreateSession {
        title: String,
        project_path: PathBuf,
        tool: Tool,
        group: Option<String>,
        dependencies: Vec<String>,        // Session IDs this depends on
        auto_start: bool,                 // Start when dependencies are met
    },

    /// Create a relationship between sessions
    CreateRelationship {
        session_a: String,
        session_b: String,
        relation_type: RelationType,
        label: Option<String>,
        schedule: Option<ScheduleConstraint>,
    },

    /// Query entity state
    QuerySession {
        session_id: String,
        components: Vec<String>,          // Which components to return
    },

    /// Subscribe to events (returns event stream)
    Subscribe {
        event_types: Vec<String>,
        filter: Option<EntityFilter>,
    },

    /// Batch operation: create a session graph
    CreateGraph {
        sessions: Vec<SessionSpec>,
        relationships: Vec<RelationshipSpec>,
    },
}
```

**CLI interface**:
```bash
# Create a session
agent-hand create --title "auth-service" --tool claude --path ~/proj/auth --group work/backend

# Create with dependency
agent-hand create --title "api-gateway" --tool claude --path ~/proj/api --depends-on auth-service --auto-start

# Create relationship
agent-hand relate auth-service api-gateway --type dependency --label "needs JWT tokens"

# Query session metrics
agent-hand query auth-service --metrics

# Subscribe to events (stream to stdout as JSONL)
agent-hand events --type status_changed --session auth-service

# Batch create from TOML spec
agent-hand graph create --spec project-plan.toml
```

### 4.2 Event Log Format

Events persisted as JSONL at `~/.agent-hand/profiles/{profile}/events.jsonl`:

```json
{"ts":"2026-03-06T10:15:30Z","type":"StatusChanged","entity":"abc123","from":"Idle","to":"Running"}
{"ts":"2026-03-06T10:15:35Z","type":"HookEventReceived","entity":"abc123","kind":"UserPromptSubmit"}
{"ts":"2026-03-06T10:17:00Z","type":"HookEventReceived","entity":"abc123","kind":{"ToolFailure":{"tool":"Bash","error":"timeout"}}}
{"ts":"2026-03-06T10:17:01Z","type":"StatusChanged","entity":"abc123","from":"Running","to":"Waiting"}
{"ts":"2026-03-06T10:20:00Z","type":"StatusChanged","entity":"abc123","from":"Waiting","to":"Running"}
{"ts":"2026-03-06T10:25:00Z","type":"StatusChanged","entity":"abc123","from":"Running","to":"Idle"}
{"ts":"2026-03-06T10:25:01Z","type":"ReactionTriggered","rule":"idle-summarize","entity":"abc123"}
```

### 4.3 Metrics Query Protocol

```rust
pub struct MetricsQuery {
    pub entity: Option<EntityId>,         // None = global
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub metrics: Vec<MetricType>,
}

pub enum MetricType {
    TotalRunningTime,
    TotalWaitingTime,
    TotalIdleTime,
    StatusTransitionCount,
    ToolUsageDistribution,
    ErrorRate,
    AverageSessionDuration,
    RelationshipUtilization,
}

pub struct MetricsResult {
    pub entity: Option<EntityId>,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub values: HashMap<MetricType, MetricValue>,
}

pub enum MetricValue {
    Duration(Duration),
    Count(u64),
    Rate(f64),
    Distribution(HashMap<String, u64>),
}
```

---

## 5. UI/UX Design

### 5.1 Analytics Dashboard

Accessible via `a` key from main view:

```
╭─ Agent Hand ─ Analytics ───────────────────────────────────────╮
│                                                                │
│  Session Activity (last 24h)                                   │
│  ┌────────────────────────────────────────────────────┐        │
│  │ auth-service  ████████░░░░████░░██████░░░░░░░░░░░│ 4.2h   │
│  │ api-gateway   ░░░░░░░░████████░░░░░░████████░░░░░│ 3.8h   │
│  │ frontend      ██████████████████░░░░░░░░░░░░░░░░░│ 2.1h   │
│  │ db-migrations ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│ 0.5h   │
│  └────────────────────────────────────────────────────┘        │
│   ████ Running  ░░░░ Idle  ████ Waiting                       │
│                                                                │
│  Tool Usage          Status Distribution     Errors            │
│  ┌──────────┐       ┌──────────────┐        ┌──────────┐     │
│  │ Claude 65%│       │ Running  40% │        │ Total: 3  │     │
│  │ Gemini 25%│       │ Waiting  25% │        │ auth:  2  │     │
│  │ Shell  10%│       │ Idle     35% │        │ api:   1  │     │
│  └──────────┘       └──────────────┘        └──────────┘     │
│                                                                │
│  [q]Back  [s]Session detail  [t]Time range  [e]Export          │
╰────────────────────────────────────────────────────────────────╯
```

### 5.2 Event Timeline View

Accessible via `e` key from session detail:

```
╭─ Event Timeline ─ auth-service ────────────────────────────────╮
│                                                                │
│  10:15:30  ● Status: Idle → Running                           │
│  10:15:35  ◇ Hook: UserPromptSubmit                           │
│  10:16:00  ◇ Hook: PermissionRequest (Bash)                   │
│  10:17:00  ✗ Hook: ToolFailure (Bash: timeout)                │
│  10:17:01  ● Status: Running → Waiting                        │
│  10:18:30  ◇ Reaction: attention_sound triggered              │
│  10:20:00  ● Status: Waiting → Running                        │
│  10:25:00  ● Status: Running → Idle                           │
│  10:25:01  ◇ Reaction: idle-summarize triggered               │
│  10:25:05  ◆ Preview updated: "Completed JWT middleware..."   │
│                                                                │
│  ● Status  ◇ Event  ✗ Error  ◆ System                        │
│  [q]Back  [f]Filter  [/]Search  [j/k]Scroll                  │
╰────────────────────────────────────────────────────────────────╯
```

### 5.3 Reaction Configuration UI

Accessible from settings:

```
╭─ Reactions ────────────────────────────────────────────────────╮
│                                                                │
│  ✓ error-alert      Status → Error    → Desktop notification  │
│  ✓ idle-summarize   Status → Idle     → AI summarize (60s cd) │
│  ✓ waiting-sound    Status → Waiting  → Play "attention"      │
│  ✗ dep-auto-start   Status → Idle     → Start dependents      │
│                                                                │
│  [Enter]Toggle  [n]New  [e]Edit  [d]Delete  [q]Back           │
╰────────────────────────────────────────────────────────────────╯
```

---

## 6. Implementation Strategy

### Phase 1: World and Entity Foundation (Week 1-2)
- Implement `World`, `Entity`, `EntityId`, component storage
- Implement `EventBus` with typed event queues
- Create `SessionIdentity`, `SessionState` components
- Bridge: Convert existing `Instance` → entity with components on load
- Bridge: Convert entity + components → `Instance` for existing rendering
- No behavior changes — existing App code continues to work

### Phase 2: Core Systems (Week 3-4)
- Implement `System` trait and `GameLoop`
- Port `StatusDetectionSystem` from current polling logic
- Port `HookReceiverSystem` from `EventReceiver`
- Implement `EventLogSystem` for persistent event recording
- Implement `PersistenceSystem` (replaces direct Storage calls)
- Current App becomes thin adapter over World

### Phase 3: Metrics and Analytics (Week 5-6)
- Implement `SessionMetrics` component
- Implement `MetricsSystem` that processes status/hook events into metrics
- Build analytics TUI view
- Build event timeline TUI view
- CLI `agent-hand query` for metrics

### Phase 4: Reaction System (Week 7)
- Implement `ReactionSystem` with rule evaluation
- Migrate sound effects from `NotificationManager` into reaction rules
- Add desktop notification action
- Add AI summarize action
- Configuration via TOML

### Phase 5: Agent Scheduling (Week 8)
- Implement `SchedulerSystem`
- `ScheduleConstraint` component on relationship entities
- CLI `agent-hand create` and `agent-hand graph` commands
- Topological ordering for dependency-based auto-start
- Event subscription via `agent-hand events`

### Phase 6: Full Migration (Week 9-10)
- Move remaining App state into components
- Remove Instance/Relationship direct usage from UI code
- UI queries World directly
- App struct becomes thin wrapper: keyboard handling → World → render

---

## 7. Dependencies

### Depends On
- **Session system** (`src/session/`): Migrated into entity + components
- **Hook system** (`src/hooks/`): Becomes `HookReceiverSystem`
- **Tmux integration** (`src/tmux/`): Becomes a World resource
- **AI Summarizer** (`src/ai/`): Used by `PreviewSystem` and `ReactionSystem`
- **Storage** (`src/session/storage.rs`): Becomes `PersistenceSystem`

### Enables
- **Canvas View** (Spec 04): Canvas reads entities and relationships from World
- **Memory System** (Spec 07): Memory becomes components; semantic analysis becomes a system
- **Collaboration** (Spec 01): Viewers and sharing become entities with presence components
- **Presence Tracking** (Spec 03): User entities with presence components
- **Fork System** (Spec 02): Fork creates child entity with ParentChild relationship entity
- **Security** (Spec 06): Access control becomes a system that validates events

### New Dependencies (crates)
- None. Custom ECS is straightforward Rust: `HashMap`, `TypeId`, `Box<dyn Any>`.
- Consider `downcast-rs` for ergonomic component downcasting (tiny crate, no deps).

---

## 8. Open Questions

1. **Migration strategy**: Big-bang vs incremental? Proposed: incremental — World wraps existing data, systems delegate to existing code, then gradually migrate. Phase 1 should have zero behavioral changes.

2. **Tick rate**: 60fps (16ms) is good for UI but excessive for systems like PersistenceSystem. Solution: `SystemSchedule::Interval` — persistence runs every 5 seconds, status detection every 500ms, rendering every tick.

3. **Event retention**: How long to keep events in the event log? Proposed: rolling log with configurable max size (default 10MB), plus daily summaries that persist indefinitely.

4. **Component access patterns**: Should systems get mutable World access or read-only with change buffers? Proposed: mutable access (simpler), since we have few entities and systems don't run in parallel.

5. **Plugin loading**: Should plugin systems be compiled Rust (loaded as dylibs) or interpreted (Lua/Rhai scripts)? Proposed: Phase 1 uses compiled Rust systems only. Scripting is a future extension.

6. **Backward compatibility**: Should the CLI still work without ECS (for users who don't want it)? Proposed: ECS is internal architecture, invisible to users. No opt-in required.

7. **Entity ID mapping**: Existing sessions have string IDs (12-char UUID prefix). Should EntityId be the same string, or should there be a mapping layer? Proposed: EntityId wraps u64 internally, with a bidirectional map to string session IDs for persistence and CLI.

8. **System ordering**: Some systems have implicit ordering (HookReceiver before Metrics, Metrics before Reactions). Should ordering be explicit (priority + dependencies) or implicit (run in registration order)? Proposed: explicit priority tiers (High/Normal/Low) with fixed order within each tier.
