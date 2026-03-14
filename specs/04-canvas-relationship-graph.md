# Canvas / Relationship Graph View — SPEC

## 1. Overview

The Canvas is a node-based graph visualization rendered entirely in the TUI. It transforms the existing tree view's linear hierarchy into a spatial layout where sessions, groups, and functions are nodes connected by typed relationship edges.

Each node displays an AI-powered ASCII preview — a structured summary of that session's current state captured from tmux pane content and processed through the connected AI provider. The canvas coexists with the existing tree view as an alternate view mode, not a replacement.

### Motivation

The tree view represents containment (folders hold sessions) but cannot express cross-cutting relationships: two sessions in different folders that depend on each other, peer collaborations, or semantic proximity. The canvas makes these invisible connections visible and navigable.

### Key Design Principles

- **TUI-native**: All rendering via ratatui widgets, no external viewers
- **Relationship-first**: Edges are first-class visual elements, not decorations
- **AI-enriched**: Node previews are generated summaries, not raw terminal dumps
- **Coexistent**: Toggle between tree and canvas; both show the same data
- **Performant**: Layout computed incrementally; rendering at 60fps on 200-node graphs

---

## 2. User Stories

### US-01: View Session Relationships Spatially
> As a developer managing 8 concurrent agent sessions across 3 projects, I want to see which sessions depend on each other so I can prioritize waiting sessions that block others.

**Acceptance**: Opening canvas shows all sessions as nodes with dependency arrows. Blocked sessions are visually distinct.

### US-02: AI Preview of Session State
> As a developer, I want to see a 2-3 line summary of what each session is doing without attaching to it, so I can make routing decisions at a glance.

**Acceptance**: Each node shows an AI-generated summary refreshed on status change. Summary captures: current activity, blockers, last tool used.

### US-03: Navigate and Interact
> As a developer, I want to move between nodes with keyboard, select a node to see details, and press Enter to attach to that session's tmux pane.

**Acceptance**: Arrow keys move focus, Enter attaches, `p` toggles preview panel, `r` shows relationships for focused node.

### US-04: Create Relationships from Canvas
> As a developer, I want to draw a relationship between two nodes by selecting them sequentially, choosing the type, and confirming.

**Acceptance**: Press `l` (link) on source node, navigate to target, select relationship type from menu, relationship persists.

### US-05: Filter and Focus
> As a developer with 30+ sessions, I want to filter the canvas to show only sessions related to my current focus area.

**Acceptance**: `/` opens search, matching nodes are highlighted, non-matching nodes fade. `f` filters to only show the connected subgraph of the selected node.

### US-06: Toggle Between Views
> As a developer, I want to switch between tree view and canvas view without losing my selection context.

**Acceptance**: `Tab` toggles views. If a session is selected in tree, it's focused in canvas and vice versa.

---

## 3. Architecture

### 3.1 Component Overview

```
┌──────────────────────────────────────────────────────┐
│                    Canvas View                        │
│                                                      │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐       │
│  │ Layout   │───>│ Renderer │───>│ ratatui  │       │
│  │ Engine   │    │          │    │ Frame    │       │
│  └────▲─────┘    └──────────┘    └──────────┘       │
│       │                                              │
│  ┌────┴─────┐    ┌──────────┐                       │
│  │ Graph    │<───│ Data     │                       │
│  │ Model    │    │ Bridge   │                       │
│  └──────────┘    └────▲─────┘                       │
│                       │                              │
└───────────────────────┼──────────────────────────────┘
                        │
              ┌─────────┴─────────┐
              │ App State         │
              │ (sessions, groups,│
              │  relationships)   │
              └───────────────────┘
```

### 3.2 Graph Model

```rust
/// Represents the canvas graph, derived from App state
pub struct CanvasGraph {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
    pub positions: HashMap<String, Position>,  // node_id → (x, y)
    pub viewport: Viewport,
    pub focused_node: Option<String>,
    pub selection: SelectionState,
}

pub struct CanvasNode {
    pub id: String,
    pub node_type: NodeType,
    pub label: String,
    pub preview: Option<NodePreview>,
    pub status: Option<Status>,           // For session nodes
    pub position: Position,               // Layout-computed
    pub size: NodeSize,                   // Computed from content
}

pub enum NodeType {
    Session { instance_id: String },
    Group { group_path: String },
    Function { name: String, session_id: String },  // Future: code-level nodes
    Project { path: PathBuf },
}

pub struct CanvasEdge {
    pub id: String,
    pub source: String,                   // node_id
    pub target: String,                   // node_id
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

pub enum EdgeType {
    Relationship(RelationType),           // Maps to existing RelationType
    GroupMembership,                       // Session belongs to group
    ParentChild,                          // Fork/sub-session
}

pub struct Viewport {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,                        // 0.5 to 2.0
    pub width: u16,
    pub height: u16,
}

pub struct Position {
    pub x: f64,
    pub y: f64,
}

pub struct NodeSize {
    pub width: u16,                       // In terminal columns
    pub height: u16,                      // In terminal rows
}
```

### 3.3 Layout Engine

The layout engine converts the graph model into positioned nodes. It runs incrementally — only repositioning when nodes or edges change.

**Algorithm**: Force-directed layout adapted for terminal grids.

```rust
pub struct LayoutEngine {
    config: LayoutConfig,
    state: LayoutState,
}

pub struct LayoutConfig {
    pub repulsion_strength: f64,          // Node-node repulsion
    pub attraction_strength: f64,         // Edge-connected attraction
    pub damping: f64,                     // Velocity damping per tick
    pub grid_snap: bool,                  // Snap to character grid
    pub iterations_per_frame: usize,      // Layout steps per render tick
    pub convergence_threshold: f64,       // Stop when below this delta
    pub group_gravity: f64,               // Pull group members together
}

pub struct LayoutState {
    pub velocities: HashMap<String, (f64, f64)>,
    pub converged: bool,
    pub iteration: usize,
}
```

**Layout modes**:
1. **Force-directed** (default): Organic layout, related nodes cluster naturally
2. **Hierarchical**: Top-down DAG layout for dependency chains
3. **Grid**: Evenly spaced, good for many unrelated sessions
4. **Manual**: User-positioned nodes, positions persisted

### 3.4 AI Preview Generation

```rust
pub struct NodePreview {
    pub summary: String,                  // 2-3 line AI summary
    pub generated_at: DateTime<Utc>,
    pub stale: bool,                      // True if session state changed since generation
    pub ascii_art: Option<String>,        // Optional: structured ASCII representation
}

/// Preview generation pipeline
pub struct PreviewGenerator {
    summarizer: Summarizer,               // Existing AI summarizer
    cache: HashMap<String, NodePreview>,
    pending: HashSet<String>,             // Sessions currently being summarized
    refresh_interval: Duration,           // Min time between re-summarizations
}
```

**Preview generation flow**:
1. Session status changes (Running → Waiting, etc.)
2. PreviewGenerator checks cache staleness
3. If stale: capture pane content (last 50 lines from tmux)
4. Send to AI summarizer (non-blocking, via existing channel)
5. On result: update cache, mark node for re-render
6. Display: Show cached summary, with "..." indicator if refreshing

**Preview prompt template**:
```
Summarize this AI coding agent session in 2-3 lines.
Session: {title} ({tool})
Status: {status}
Terminal output (last 50 lines):
{pane_content}

Format: Line 1: Current activity. Line 2: Key files/areas. Line 3: Blockers (if any).
```

### 3.5 Data Bridge

The Data Bridge converts App state into CanvasGraph, keeping them synchronized.

```rust
impl DataBridge {
    /// Rebuild graph from current app state
    pub fn build_graph(
        sessions: &[Instance],
        groups: &GroupTree,
        relationships: &[Relationship],
    ) -> CanvasGraph;

    /// Incremental update: apply delta changes
    pub fn apply_changes(
        graph: &mut CanvasGraph,
        changes: &[StateChange],
    );
}

pub enum StateChange {
    SessionAdded(String),
    SessionRemoved(String),
    SessionStatusChanged(String, Status),
    RelationshipAdded(Relationship),
    RelationshipRemoved(String),
    GroupChanged(String),
}
```

---

## 4. Protocol / API

### 4.1 Node Rendering Protocol

Each node renders as a bordered box in the terminal:

```
┌─[Claude]──running──────────┐
│ auth-service               │
│ Implementing JWT middleware│
│ src/auth/jwt.rs            │
└────────────────────────────┘
```

**Node anatomy**:
```
┌─[{tool}]──{status}──{label_color}─┐
│ {title}                            │  ← Line 1: Session title
│ {preview_line_1}                   │  ← Line 2: AI summary
│ {preview_line_2}                   │  ← Line 3: Key files / blockers
└────────────────────────────────────┘
```

**Collapsed node** (zoomed out or compact mode):
```
[●auth-svc]
```

**Group node**:
```
╔═work/frontend═══╗
║  3 sessions     ║
║  1 running      ║
╚═════════════════╝
```

### 4.2 Edge Rendering Protocol

Edges drawn with Unicode box-drawing characters:

| Edge Type | Symbol | Example |
|-----------|--------|---------|
| Dependency | `───▶` | `[A]───▶[B]` (A depends on B) |
| ParentChild | `───┤` | `[parent]───┤[child]` |
| Peer | `═══` | `[A]═══[B]` |
| Collaboration | `~~~` | `[A]~~~[B]` |
| GroupMembership | `│` | Vertical line from group to member |

**Edge routing**: Edges avoid crossing nodes. Simple orthogonal routing with:
1. Source port selection (nearest edge of source node to target)
2. Manhattan routing (horizontal + vertical segments)
3. Crossing minimization (swap port sides if crossing detected)

### 4.3 Canvas Actions

| Action | Key | Description |
|--------|-----|-------------|
| Move focus | `h/j/k/l` or arrows | Navigate between nodes |
| Attach | `Enter` | Attach to focused session's tmux pane |
| Toggle preview | `p` | Show/hide detailed preview panel |
| Create link | `L` | Start link mode: select target, choose type |
| Delete link | `D` | Delete relationship on focused edge |
| Filter | `/` | Search/filter nodes |
| Focus subgraph | `f` | Show only connected component of focused node |
| Reset view | `0` | Reset zoom and position |
| Zoom in/out | `+`/`-` | Adjust zoom level |
| Pan | `H/J/K/L` (shift) | Pan viewport |
| Layout mode | `1-4` | Switch layout algorithm |
| Toggle view | `Tab` | Switch between tree and canvas |
| Refresh previews | `R` | Force re-summarize all visible nodes |

### 4.4 Persisted Canvas State

```rust
pub struct CanvasState {
    pub layout_mode: LayoutMode,
    pub manual_positions: HashMap<String, Position>,  // Only for manual layout
    pub viewport: Viewport,
    pub collapsed_nodes: HashSet<String>,
    pub hidden_edge_types: HashSet<EdgeType>,
}
```

Persisted alongside `StorageData` in `sessions.json` (new field `canvas_state`).

---

## 5. UI/UX Design

### 5.1 Full Canvas View (no preview panel)

```
╭─ Agent Hand ─ Canvas ──────────────────────────────────────────╮
│                                                                │
│   ╔═work/backend══╗                                           │
│   ║ 4 sessions    ║                                           │
│   ╚═══════╤═══════╝                                           │
│           │                                                    │
│     ┌─────┼─────────────────────────┐                         │
│     │     │                         │                         │
│  ┌─[C]─running───┐   ┌─[C]─waiting──┐   ┌─[G]─idle────┐    │
│  │ auth-service   │──▶│ api-gateway  │═══│ frontend    │    │
│  │ JWT middleware │   │ Blocked: auth│   │ React hooks │    │
│  │ src/auth/jwt   │   │ needs token  │   │ Dashboard   │    │
│  └────────────────┘   └──────────────┘   └─────────────┘    │
│           │                                                    │
│           │ ← dependency                                      │
│           ▼                                                    │
│  ┌─[C]─idle──────┐                                           │
│  │ db-migrations  │                                           │
│  │ Schema done    │                                           │
│  │ 14 migrations  │                                           │
│  └────────────────┘                                           │
│                                                                │
│  [Tab]Tree [L]Link [/]Search [f]Focus [+/-]Zoom [?]Help      │
╰────────────────────────────────────────────────────────────────╯
```

### 5.2 Canvas with Preview Panel (right side)

```
╭─ Canvas ───────────────────────────┬─ Preview ───────────────╮
│                                    │ auth-service             │
│  ┌─[C]─running───┐                │ ─────────────────────── │
│  │ auth-service   │◀── focused     │ Tool: Claude Code       │
│  │ JWT middleware │                │ Status: Running (2m)    │
│  └───────┬────────┘                │ Path: ~/proj/auth       │
│          │                         │                         │
│          ▼                         │ Summary:                │
│  ┌─[C]─waiting───┐                │ Implementing JWT token  │
│  │ api-gateway   │                │ validation middleware.  │
│  │ Blocked: auth │                │ Currently writing tests │
│  └───────────────┘                │ for refresh token flow. │
│                                    │                         │
│                                    │ Relationships:          │
│                                    │  → api-gateway (dep)   │
│                                    │  → db-migrations (dep) │
│                                    │  ═ frontend (peer)     │
│                                    │                         │
│                                    │ Last Activity: 30s ago │
╰────────────────────────────────────┴─────────────────────────╯
```

### 5.3 Status Colors and Node Borders

| Status | Border Color | Indicator |
|--------|-------------|-----------|
| Running | Green | `●` spinning animation |
| Waiting | Yellow | `◆` blinking |
| Idle | Gray/dim | `○` static |
| Error | Red | `✗` static |
| Starting | Cyan | `◌` pulsing |

Tool badges: `[C]` Claude, `[G]` Gemini, `[O]` OpenCode, `[X]` Codex, `[S]` Shell

### 5.4 Link Creation Flow

```
1. Focus source node → Press [L]
   Status bar: "Link mode: select target node"
   Source node border: highlighted blue

2. Navigate to target node → Press [Enter]
   Dialog appears:
   ┌─ Create Relationship ─────────┐
   │ auth-service → api-gateway    │
   │                               │
   │ Type:                         │
   │  [1] Dependency (A→B)        │
   │  [2] Peer (A═B)             │
   │  [3] Collaboration (A~B)    │
   │  [4] Custom                  │
   │                               │
   │ Label (optional): ________   │
   │                               │
   │ [Enter] Create  [Esc] Cancel │
   └───────────────────────────────┘

3. Select type → Confirm
   Edge appears, layout recomputes
```

---

## 6. Implementation Strategy

### Phase 1: Static Graph Rendering (Week 1-2)
- Implement `CanvasGraph` data model
- Implement `DataBridge` to convert App state → graph
- Implement basic force-directed layout (no animation)
- Render nodes as bordered boxes with title + status
- Render edges as straight lines (no routing)
- Add `Tab` toggle between tree and canvas
- No AI previews yet — show title + status only

### Phase 2: Navigation and Interaction (Week 3)
- Implement viewport panning and focus navigation
- Node selection and detail panel
- `Enter` to attach to session
- Zoom levels (collapsed vs expanded nodes)
- Keyboard shortcuts for all canvas actions

### Phase 3: Edge Routing and Layout Modes (Week 4)
- Manhattan edge routing with crossing minimization
- Hierarchical layout algorithm
- Grid layout
- Manual positioning with persistence
- Edge type visual differentiation

### Phase 4: AI Previews (Week 5)
- Integrate with existing `Summarizer`
- Preview generation pipeline with caching
- Stale detection and refresh triggers
- Preview panel (right side split)

### Phase 5: Relationship Management (Week 6)
- Link creation flow (L → select → type → confirm)
- Link deletion
- Filter/focus subgraph
- Search within canvas

---

## 7. Dependencies

### Depends On
- **Session system** (`src/session/`): Node data source
- **Relationship system** (`src/session/relationships.rs`): Edge data source
- **AI Summarizer** (`src/ai/summarize.rs`): Preview generation
- **TUI framework** (ratatui): Rendering primitives
- **Storage** (`src/session/storage.rs`): Canvas state persistence

### Enables
- **ECS Runtime** (Spec 05): Canvas becomes a visual layer over ECS entities
- **Memory System** (Spec 07): Semantic relationships visualized as edges
- **Presence Tracking** (Spec 03): Viewer cursors on canvas nodes
- **Fork System** (Spec 02): Fork operations initiated from canvas

### New Dependencies (crates)
- None required. All rendering via ratatui's existing `Canvas` widget and custom drawing.

---

## 8. Open Questions

1. **Maximum node count**: At what point should we switch to clustering/aggregation? Proposed threshold: 50 visible nodes → auto-cluster by group.

2. **Layout persistence**: Should force-directed positions be saved, or only manual positions? Force-directed re-layout on startup could be disorienting if graph hasn't changed.

3. **Real-time layout**: Should layout animate (smooth node movement) or snap? Animation is more natural but costs CPU. Proposed: animate for first 30 iterations, then snap.

4. **Edge label placement**: Where to show relationship labels on edges? Options: midpoint of edge, near source port, or only on hover/focus.

5. **Group nodes**: Should groups be collapsible containers (showing member nodes inside) or summary nodes (single node with member count)? Proposed: both modes, toggled per-group.

6. **AI preview cost**: Summarizing all visible nodes on every status change could be expensive. Rate limiting strategy: max 3 concurrent summarizations, 30-second cooldown per node.

7. **Mouse support**: Should canvas support mouse click to select and drag to pan? The app already has configurable mouse capture. Proposed: yes, when mouse_capture is enabled.
