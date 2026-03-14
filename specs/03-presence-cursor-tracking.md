# Presence & Cursor Tracking — SPEC

## 1. Overview

### Problem

In the current collaboration system, the host has minimal awareness of viewers:
- A viewer count badge shows how many people are connected
- A presence list shows viewer names and permission levels
- But the host has **no idea what each viewer is looking at**

This creates a disconnected experience — the host doesn't know if viewers are following along, scrolled back reading history, confused by a specific section, or even paying attention. Viewers also can't see where other viewers are focused.

### Solution

Implement **Feishu/Lark-style collaborative awareness** for terminal sessions. Each viewer reports their viewport position (which lines they can see), and this information is visualized for all participants. Think of it as "collaborative document cursors" adapted for terminals:

- **Viewer → Server**: Reports scroll position (line range visible in viewport)
- **Server → Host**: Aggregated presence overlay showing where each viewer is focused
- **Server → Viewers**: Other viewers' positions shown as colored markers in the scrollbar

### Design Principles

1. **Non-intrusive**: Presence indicators must not interfere with terminal content
2. **Bandwidth-minimal**: Position updates are small, throttled, and delta-compressed
3. **Privacy-respecting**: Viewers opt into presence tracking (enabled by default, can disable)
4. **Degrades gracefully**: If presence data is missing or stale, UI simply hides indicators

---

## 2. User Stories

### US-1: Host sees viewer focus
> As a **host**, when 3 viewers are connected, I want to see where each viewer's viewport is positioned relative to my terminal, so I know if they're following along or reading old output.

### US-2: Know when viewers fall behind
> As a **host** doing a live demo, when I run a command and a viewer is still scrolled back reading previous output, I want a subtle indicator so I can pause and let them catch up.

### US-3: Viewer sees others' positions
> As a **viewer**, I want to see where other viewers are focused in the scrollbar gutter, so I can tell if everyone is looking at the same thing or if someone noticed something I missed.

### US-4: Follow another viewer
> As a **viewer**, when I see another viewer focused on a specific section of output, I want to click their marker to jump to the same position.

### US-5: Host sees "all eyes on me"
> As a **host**, when all viewers are in LIVE mode watching my current terminal, I want a simple "all synced" indicator instead of cluttered individual markers.

### US-6: Privacy control
> As a **viewer**, I want to disable presence tracking so the host and other viewers can't see my scroll position.

---

## 3. Architecture

### 3.1 Coordinate System

Presence tracking requires a **shared coordinate system** between host and viewers. This system is built on the sequence numbers defined in SPEC-01 (Scrollable Viewer):

```
Presence Position = {
    // What the viewer can see
    viewport_top_seq: u64,     // Sequence number of topmost visible line
    viewport_bottom_seq: u64,  // Sequence number of bottommost visible line

    // Viewer's mode
    mode: "live" | "scroll",   // Whether synced to host or browsing independently
}
```

**Why sequence numbers?** Lines in the terminal ring buffer have stable, monotonically increasing sequence numbers. This gives all participants a shared reference frame regardless of their viewport size, terminal dimensions, or scroll position.

### 3.2 Data Flow

```
┌──────────┐                    ┌──────────────┐                    ┌──────────┐
│ Viewer A │──PresenceUpdate──→ │              │──PresenceBroadcast→│  Host    │
│          │                    │ Relay Server │                    │          │
│ Viewer B │──PresenceUpdate──→ │              │──PresenceBroadcast→│ Viewer A │
│          │                    │  (aggregates │                    │          │
│ Viewer C │──PresenceUpdate──→ │   & batches) │──PresenceBroadcast→│ Viewer B │
└──────────┘                    └──────────────┘                    └──────────┘
```

### 3.3 Server-Side Aggregation

The relay server **does not simply relay** individual presence updates. Instead, it:

1. **Collects** updates from all viewers (stored per-viewer in the Room struct)
2. **Batches** updates into periodic broadcasts (every 500ms)
3. **Compresses** the broadcast — only sends positions that changed since last broadcast
4. **Filters** — viewers who opted out of presence are excluded

This prevents N viewers from generating N² presence messages.

### 3.4 Component Diagram

```
┌─ Relay Server Room ─────────────────────────────────┐
│                                                      │
│  viewers: HashMap<ViewerId, ViewerState>             │
│                                                      │
│  ViewerState {                                       │
│      display_name: String,                           │
│      color: String,          // assigned color       │
│      permission: Permission,                         │
│      presence: Option<PresencePosition>,             │
│      presence_visible: bool, // opt-in/out           │
│      last_update: Instant,   // staleness tracking   │
│  }                                                   │
│                                                      │
│  presence_broadcast_task:                            │
│    every 500ms → collect all ViewerState.presence    │
│    → diff against last broadcast                     │
│    → if changed, send PresenceBroadcast to all       │
│                                                      │
└──────────────────────────────────────────────────────┘
```

### 3.5 Color Assignment

Each viewer gets a unique, visually distinct color assigned server-side on join. Colors are drawn from a palette designed for terminal readability:

```
Palette (8 colors, recycled with suffix for >8 viewers):
#FF6B6B (coral)      → Viewer A
#4ECDC4 (teal)       → Viewer B
#45B7D1 (sky blue)   → Viewer C
#96CEB4 (sage)       → Viewer D
#FFEAA7 (cream)      → Viewer E
#DDA0DD (plum)       → Viewer F
#98D8C8 (mint)       → Viewer G
#F7DC6F (gold)       → Viewer H
```

Colors are assigned round-robin from the palette, skipping any color too similar to the terminal background (dark themes: skip dark colors; light themes: skip light colors).

---

## 4. Protocol / API

### 4.1 New Control Messages

```rust
// Viewer → Server: Report current viewport position
// Sent on: scroll events (throttled), mode changes, viewport resize
PresenceUpdate {
    viewport_top_seq: u64,
    viewport_bottom_seq: u64,
    mode: "live" | "scroll",
    /// Set to false to hide presence from others
    visible: bool,
}

// Server → All: Batched presence state for all viewers
// Sent every 500ms (only when changed)
PresenceBroadcast {
    /// All viewer positions (excluding opted-out viewers)
    viewers: Vec<ViewerPresence>,
    /// Timestamp of this broadcast (for staleness detection)
    ts: u64,
}

// Individual viewer presence entry
ViewerPresence {
    viewer_id: String,
    display_name: String,
    color: String,              // hex color code
    viewport_top_seq: u64,
    viewport_bottom_seq: u64,
    mode: "live" | "scroll",
}
```

### 4.2 Update Throttling Rules

| Event | Throttle | Rationale |
|-------|----------|-----------|
| Scroll (continuous) | Max 1 update per 200ms | Prevent flood during fast scrolling |
| Mode change (live↔scroll) | Immediate | Important state transition |
| Viewport resize | Immediate, then 200ms throttle | Size matters for rendering |
| Visibility toggle | Immediate | Privacy action, don't delay |

### 4.3 Staleness Policy

- If a viewer hasn't sent a `PresenceUpdate` in **10 seconds**, their position is marked stale
- Stale positions are rendered with reduced opacity (50% alpha)
- After **30 seconds** of no update, the viewer's position is removed from broadcasts
- The viewer is still listed in `PresenceList` (they're connected), just not in `PresenceBroadcast`

### 4.4 Integration with Existing Messages

The existing `ViewerJoined` message is extended:

```rust
ViewerJoined {
    viewer_id: String,
    display_name: String,
    permission: String,
    color: String,          // NEW: assigned presence color
}
```

The existing `PresenceList` message is extended:

```rust
PresenceList {
    viewers: Vec<ViewerInfo>,
}

// ViewerInfo extended:
ViewerInfo {
    viewer_id: String,
    display_name: String,
    permission: String,
    color: String,                          // NEW
    presence: Option<ViewerPresence>,       // NEW: current position if available
}
```

---

## 5. UI/UX Design

### 5.1 Browser Viewer — Scrollbar Gutter

The primary presence visualization is **colored markers in the scrollbar gutter**:

```
Terminal content                          Scrollbar
┌────────────────────────────────────┐   ┌──┐
│ $ cargo build                      │   │  │
│    Compiling foo v0.1.0            │   │  │
│ error[E0308]: mismatched types     │   │▓▓│ ← Viewer B (teal) viewport
│   --> src/main.rs:42:5             │   │▓▓│
│ $ cargo test                       │   │  │
│ running 12 tests                   │   │░░│ ← Viewer C (sky blue) viewport
│ test auth::login ... ok            │   │░░│
│ test auth::logout ... FAILED       │   │  │
│                                    │   │  │
│ $ vim src/main.rs                  │   │██│ ← You are here
│ ...editing...                      │   │██│
└────────────────────────────────────┘   └──┘
```

**Marker rendering:**
- Each viewer's viewport is shown as a colored bar segment in the scrollbar track
- Bar height is proportional to their viewport size relative to total history
- Markers are semi-transparent (40% opacity) to avoid obscuring scroll position
- Your own position is shown as the standard scrollbar thumb (not a colored marker)

### 5.2 Browser Viewer — Presence Legend

A compact legend appears below the terminal when presence data is available:

```
┌────────────────────────────────────────────────┐
│ 👁 Alice (●LIVE)  Bob (↑ line 234)  Carol (●LIVE) │
└────────────────────────────────────────────────┘
```

| Symbol | Meaning |
|--------|---------|
| `●LIVE` | Viewer is in live mode, synced to host |
| `↑ line N` | Viewer is scrolled back to approximately line N |
| Dimmed name | Viewer presence is stale (>10s since update) |
| Hidden | Viewer opted out of presence |

**Interaction:**
- Click a viewer's name → jump to their viewport position
- Hover → tooltip with exact line range

### 5.3 Host TUI — Presence Indicators

For the host running in the terminal (not browser), presence is shown in the status bar:

```
┌─ Agent Hand ─────────────────────────────────────────┐
│                                                       │
│  [session content here]                               │
│                                                       │
├───────────────────────────────────────────────────────┤
│ 👁 3 viewers │ ●● Alice, Bob (LIVE) │ ↑ Carol (-42)  │
└───────────────────────────────────────────────────────┘
```

**Status bar format:**
- `●` colored dot + name for each viewer
- `LIVE` label for synced viewers
- `↑ -N` showing how many lines behind a scrolled-back viewer is (relative to host position)
- When all viewers are LIVE: simplified `👁 3 viewers · all synced`

### 5.4 "All Synced" Optimization

When all viewers are in LIVE mode watching the same terminal position:
- Host sees: `👁 3 viewers · all synced ✓`
- Viewers see: no scrollbar markers (everyone at same position)
- This is the common case and should be visually minimal

### 5.5 Keybindings (Browser Viewer)

| Key | Action |
|-----|--------|
| `P` | Toggle own presence visibility (opt in/out) |
| `1-9` | Jump to viewer N's position (from legend) |

---

## 6. Implementation Strategy

### Phase 1: Presence Protocol (relay-server)

**Estimated scope**: ~250 LOC

1. **Extend ViewerState in Room**
   - Add `presence: Option<PresencePosition>`, `color: String`, `presence_visible: bool`, `last_presence_update: Instant`

2. **Color assignment**
   - Assign color from palette on viewer join
   - Include in `ViewerJoined` message

3. **PresenceUpdate handling**
   - Parse `PresenceUpdate` messages in viewer relay loop
   - Store in room's ViewerState
   - Validate: sequence numbers must be within available range, mode must be valid

4. **PresenceBroadcast task**
   - Spawn per-room task (alongside existing broadcast)
   - Every 500ms: collect all non-stale, visible viewer positions
   - Diff against last broadcast; skip if unchanged
   - Broadcast `PresenceBroadcast` to all connected participants

5. **Staleness cleanup**
   - In broadcast task: mark viewers stale after 10s, remove after 30s

### Phase 2: Browser Viewer UI

**Estimated scope**: ~400 LOC (JavaScript)

1. **Scrollbar gutter rendering**
   - Create a custom scrollbar track overlay on xterm.js container
   - Map sequence numbers to scrollbar pixel positions
   - Render colored rectangles for each viewer's viewport

2. **Presence legend bar**
   - Add DOM element below terminal
   - Update on each `PresenceBroadcast`
   - Handle click-to-jump interaction

3. **Send PresenceUpdate**
   - Hook into xterm.js scroll events
   - Throttle to max 5 updates/second
   - Map viewport position to sequence numbers (requires knowing which seq is at which scroll position)
   - Send on mode transitions (live ↔ scroll)

4. **Privacy toggle**
   - Keyboard shortcut `P` to toggle visibility
   - Persist preference in localStorage
   - Send `PresenceUpdate { visible: false }` when opted out

### Phase 3: Host TUI Integration

**Estimated scope**: ~150 LOC (Rust)

1. **Receive PresenceBroadcast in RelayClient**
   - Parse and store viewer positions
   - Expose via `RelayClient::viewer_presences()`

2. **Render in status bar**
   - Add presence rendering to the collaboration status area
   - Show dots, names, and relative scroll positions
   - "All synced" shorthand when everyone is LIVE

### Phase 4: Polish & Optimization

1. **Delta compression**: Only send changed positions in PresenceBroadcast
2. **Adaptive broadcast rate**: Reduce to 1s when no scrolling activity
3. **Viewer-to-viewer following**: "Follow Alice" mode that auto-scrolls to match another viewer
4. **Presence history**: Brief animation showing where a viewer was → where they scrolled to

---

## 7. Dependencies

### Depends On

- **SPEC-01: Scrollable Viewer** — The sequence number coordinate system is the foundation for presence positions. Without shared seq numbers, there's no way to express "where" a viewer is looking.
- Existing `PresenceList` / `ViewerJoined` / `ViewerLeft` protocol messages

### Enables

- **Follow Mode**: Viewers auto-scrolling to match another participant's position
- **Attention Heatmap**: Aggregate presence data to show which parts of output attracted the most viewer attention
- **Host Pacing**: The host's tooling could auto-pause output when viewers are significantly behind
- **Session Analytics**: Track engagement patterns across collaboration sessions

### New Dependencies

None — this feature uses existing WebSocket infrastructure and requires no new crates.

---

## 8. Open Questions

### OQ-1: Sequence Number Mapping in Browser
**Question**: The browser viewer uses xterm.js row indices (0-based from scrollback start), while the server uses sequence numbers. How to map between them?

**Recommendation**: The viewer maintains a mapping table: when it receives binary frames or HistoryResponse lines, it records `{ xterm_row → seq }`. On scroll events, it looks up the seq for the topmost/bottommost visible xterm row. This mapping is built incrementally as content arrives.

**Alternative**: The server could include seq numbers in binary frames (as a prefix byte). Rejected because it would break the zero-overhead binary pipeline.

### OQ-2: Presence Broadcast Scope
**Question**: Should the host also send presence updates (their terminal viewport), or is host position always "live"?

**Recommendation**: The host's position is implicitly the latest content (tail of the ring buffer). No need for the host to send presence updates. The `tail_seq` from `HistoryInfo` effectively communicates where the host "is."

### OQ-3: Viewer Count vs. Presence Count
**Question**: If some viewers opt out of presence, should the presence legend show them as "N hidden" or simply omit them?

**Recommendation**: Show `"+ N hidden"` suffix when opted-out viewers exist. This avoids confusion where the viewer count says 5 but only 3 presence markers appear.

### OQ-4: Scrollbar Gutter Feasibility with xterm.js
**Question**: xterm.js doesn't natively support custom scrollbar gutter rendering. How to implement the presence markers?

**Options**:
1. **CSS overlay**: Position a transparent `<canvas>` over the xterm.js scrollbar area. Render markers on canvas. Requires careful z-index management.
2. **Custom scrollbar**: Disable xterm.js native scrollbar, implement custom scrollbar with presence markers. More control but more code.
3. **Side gutter**: Add a narrow column to the right of the terminal (outside xterm.js) that acts as a minimap with presence markers. Simpler to implement.

**Recommendation**: Option 3 (side gutter). It avoids fighting xterm.js internals, is easier to style, and provides more rendering space for presence information. The gutter can also serve as a minimap in future iterations.

### OQ-5: Privacy Defaults
**Question**: Should presence tracking be opt-in or opt-out?

**Recommendation**: **Opt-out** (enabled by default). Presence tracking is the primary value proposition of this feature. If it's opt-in, adoption will be low and the host won't see useful data. The toggle is prominently documented and easy to access (`P` key).

### OQ-6: Maximum Useful Viewer Count for Presence
**Question**: At what viewer count does individual presence tracking become noise?

**Recommendation**: Above **8 viewers**, switch from individual markers to a **heat map** visualization: aggregate positions into density bands rather than showing per-viewer markers. Below 8, individual colored markers work well. This threshold is configurable.

### OQ-7: Terminal Dimensions Mismatch
**Question**: Host terminal is 120x40, but a viewer's browser is only 80x24. Their "viewport" covers different amounts of content. How to handle?

**Recommendation**: Presence positions are expressed in **sequence numbers** (logical lines), not in rows. A viewer with a smaller viewport simply has a narrower range (`viewport_bottom_seq - viewport_top_seq` is smaller). The visual representation in the scrollbar gutter correctly shows their smaller viewport as a shorter bar segment. No special handling needed.
