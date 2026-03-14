# Collaboration V2 — Scrollable Viewer — SPEC

## 1. Overview

### Problem

The current collaboration viewer is **live-only**: guests see exactly what the host sees in real-time, with no ability to scroll back through previous output. This is a critical limitation because:

- Terminal sessions produce dense output (build logs, test runs, code diffs) that scrolls past faster than a viewer can read
- A viewer joining mid-session misses all context before their join point (late-join only gets the last snapshot — a single screen)
- Any network lag that drops binary frames creates permanent gaps in the viewer's experience
- Host and viewer are **locked in sync** — the viewer cannot pause to study output while the host keeps working

### Solution

Add independent scrollback capability for viewers, backed by a server-side terminal history buffer. Viewers can scroll freely through accumulated terminal output while the host continues working. The system distinguishes between **live mode** (synced to host) and **scroll mode** (viewer browsing history independently).

### Design Principles

1. **Bandwidth-efficient**: Don't re-transmit data the viewer already received
2. **Memory-bounded**: Ring buffer with configurable size, not unbounded growth
3. **Latency-preserving**: Scrollback must not degrade live-stream performance
4. **Architecture-compatible**: Build on existing pipe-pane → binary frame → broadcast pipeline

---

## 2. User Stories

### US-1: Scroll back through missed output
> As a **viewer**, when I join a session where a build just finished, I want to scroll up to see the build output and error messages, so I can understand what the host is debugging.

### US-2: Read at my own pace
> As a **viewer** watching a live coding session, when the host runs a test suite that produces 200 lines of output, I want to pause and scroll up to read specific test failures while the host continues working.

### US-3: Return to live
> As a **viewer** who scrolled back to read a stack trace, I want a clear way to snap back to the live terminal position, so I can re-sync with the host.

### US-4: Late-join with context
> As a **viewer** joining a session 10 minutes in, I want to see recent terminal history (not just the current screen), so I have context for what the host is doing.

### US-5: Host awareness of viewer scroll position
> As a **host**, I want to know when a viewer is scrolled back (not watching live), so I know they might miss my current actions. *(Feeds into SPEC-03: Presence & Cursor Tracking)*

### US-6: Bounded resource usage
> As a **relay server operator**, I want terminal history to be bounded in memory, so one long-running session doesn't consume unbounded RAM.

---

## 3. Architecture

### 3.1 Approach Evaluation

Three candidate approaches were considered:

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| **A. tmux capture-pane with history** | Simple, uses tmux native scrollback; host-side only | Requires host round-trip per scroll request; 100-300ms latency per page; captures only tmux-rendered output | ❌ Too slow for interactive scrolling |
| **B. Server-side terminal ring buffer** | Low latency; works for all viewers; no host load; bounded memory | Relay server must buffer bytes; adds server memory usage | ✅ **Selected** |
| **C. Replay protocol messages** | Perfect fidelity; viewers reconstruct full history | Massive bandwidth on late-join; complex seek logic; binary frames have no structure | ❌ Impractical for large histories |

### 3.2 Selected: Server-Side Terminal Ring Buffer

```
┌──────────┐        binary frames        ┌──────────────┐       live stream      ┌──────────┐
│   Host   │ ──────────────────────────→  │ Relay Server │ ─────────────────────→  │  Viewer  │
│ (tmux    │                              │              │                         │ (xterm.js│
│  pipe-   │   snapshot (5s interval)     │  ┌────────┐  │   history chunk         │  or TUI) │
│  pane)   │ ──────────────────────────→  │  │Terminal │  │ ←────────────────────── │          │
└──────────┘                              │  │ Ring    │  │ ──────────────────────→ │          │
                                          │  │ Buffer  │  │   (on scroll request)  │          │
                                          │  └────────┘  │                         └──────────┘
                                          └──────────────┘
```

### 3.3 Ring Buffer Design

The relay server maintains a **virtual terminal emulator** per room that processes all incoming binary frames. This provides:

1. **Line-structured history**: Raw PTY bytes are fed through a `vt100` parser to extract logical terminal lines
2. **Ring buffer storage**: Completed lines stored in a fixed-size ring buffer
3. **Sequence numbering**: Each line gets a monotonically increasing sequence number for random access
4. **ANSI state tracking**: Each stored line includes its ANSI formatting state at line start (so rendering is independent)

```
Ring Buffer (per room):
┌─────────────────────────────────────────────┐
│ capacity: 10,000 lines (configurable)       │
│ head_seq: 45,230  (oldest available line)   │
│ tail_seq: 55,229  (newest line)             │
│                                             │
│ [45230] "$ cargo build 2>&1"           \n   │
│ [45231] "   Compiling foo v0.1.0"      \n   │
│ [45232] "error[E0308]: mismatched..."  \n   │
│ ...                                         │
│ [55229] "$ █"  (current line, partial) \n   │
└─────────────────────────────────────────────┘
```

### 3.4 Component Interaction

```
Host binary frame arrives
        │
        ▼
┌─ Relay Server ──────────────────────────────────┐
│                                                  │
│  1. Broadcast to all viewers (unchanged)         │
│  2. Feed bytes into room's VT100 parser          │
│  3. On newline → push completed line to ring buf │
│  4. Update tail_seq                              │
│                                                  │
│  On HistoryRequest from viewer:                  │
│  5. Read requested line range from ring buffer   │
│  6. Send HistoryResponse with line data          │
│                                                  │
└──────────────────────────────────────────────────┘
```

### 3.5 Viewer State Machine

```
                  ┌──────────┐
                  │          │
         ┌───────│   LIVE   │◄──────────────┐
         │       │  (synced) │               │
         │       └──────────┘               │
         │            │                      │
         │  scroll up │               snap-to│
         │            ▼               -live  │
         │       ┌──────────┐               │
         │       │  SCROLL  │───────────────┘
         │       │ (browsing)│
         │       └──────────┘
         │            │
         │  disconnect│
         │            ▼
         │       ┌──────────┐
         └──────►│DISCONNECT│
                 └──────────┘
```

**LIVE mode**: Viewer receives and renders all binary frames in real-time (current behavior). Scrollbar tracks bottom of terminal.

**SCROLL mode**: Viewer has scrolled up. Binary frames still arrive and are buffered client-side but the viewport is detached. A "LIVE ↓" indicator appears. The viewer can request history chunks from the server for content that arrived before they joined or that they missed.

---

## 4. Protocol / API

### 4.1 New Control Messages

Add to existing `ControlMessage` enum:

```rust
// Viewer → Server: Request historical lines
HistoryRequest {
    /// Starting sequence number (inclusive).
    /// If None, request the latest N lines (tail).
    from_seq: Option<u64>,
    /// Number of lines to fetch (capped server-side at 500)
    count: u16,
    /// Direction: "backward" (scrolling up) or "forward" (scrolling down)
    direction: "backward" | "forward",
    /// Client-generated request ID for matching responses
    request_id: String,
}

// Server → Viewer: Historical line data
HistoryResponse {
    request_id: String,
    /// The lines, each with sequence number and content
    lines: Vec<HistoryLine>,
    /// Sequence range available on server [head_seq, tail_seq]
    available_range: (u64, u64),
    /// Whether more lines exist in the requested direction
    has_more: bool,
}

// Server → Viewer: Metadata about buffer state (sent on join + periodically)
HistoryInfo {
    /// Total lines currently in buffer
    total_lines: u64,
    /// Sequence number range [oldest, newest]
    available_range: (u64, u64),
    /// Ring buffer capacity
    capacity: u64,
}
```

### 4.2 HistoryLine Format

```rust
struct HistoryLine {
    /// Monotonically increasing sequence number
    seq: u64,
    /// Line content with ANSI escape codes preserved
    content: String,
    /// Timestamp when line was completed (epoch millis)
    ts: u64,
}
```

### 4.3 Sequence Number Semantics

- Sequence numbers are **per-room**, monotonically increasing, never reset
- Start at 0 when room is created
- Increment by 1 per completed line (newline-terminated)
- The **current partial line** (not yet newline-terminated) is NOT in the buffer; it's part of the live stream
- After ring buffer wraps, `head_seq` advances (old lines evicted)

### 4.4 Wire Efficiency

- History responses use **text frames** (JSON) since they're infrequent and need structure
- Live binary stream is **unchanged** — no overhead added to the hot path
- HistoryResponse lines are sent as a single JSON message (not streamed line-by-line)
- Client caches received history chunks to avoid re-requesting

### 4.5 Late-Join Enhancement

Current: Viewer gets `last_snapshot` (one screen of terminal state).

New: After `AuthResult`, server sends:

1. `HistoryInfo` — tells viewer how much history is available
2. `Snapshot` — current terminal state (existing behavior)
3. Viewer can then `HistoryRequest` backwards from snapshot to get preceding context

---

## 5. UI/UX Design

### 5.1 Browser Viewer (xterm.js)

**Scroll interaction:**
- Mouse wheel / trackpad scroll triggers transition from LIVE → SCROLL mode
- xterm.js has native scrollback support (`scrollback` option)
- Client-side buffer augmented with server-fetched history

**Visual indicators:**

```
┌─────────────────────────────────────────┐
│ $ cargo test                            │  ← history (fetched from server)
│ running 42 tests                        │
│ test auth::login ... ok                 │
│ test auth::logout ... FAILED            │
│ ...                                     │
│─────── scrollbar shows position ────────│
│                                         │
│                                         │
│ ┌─────────────────────────────────────┐ │
│ │  ↓ LIVE  (click or press End)      │ │  ← floating indicator when scrolled back
│ └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

**Keybindings:**
| Key | Action |
|-----|--------|
| `Scroll Up` | Enter SCROLL mode, fetch history if needed |
| `Scroll Down` | Move toward live position |
| `End` / click "LIVE" | Snap to live mode |
| `Home` | Jump to oldest available history |
| `Page Up/Down` | Scroll by screenful |

**Fetch strategy:**
- Prefetch: When viewer scrolls within 50 lines of cached boundary, request next chunk
- Chunk size: 200 lines per request
- Cache: Store fetched lines in client-side map keyed by seq number
- Dedup: Don't re-request lines already in client cache

### 5.2 TUI Viewer (Future)

For a future TUI-based viewer (native Rust client), the same protocol applies. The TUI would use its own virtual terminal buffer with identical LIVE/SCROLL state machine.

### 5.3 "LIVE" Indicator Behaviors

| State | Indicator | Color |
|-------|-----------|-------|
| LIVE (synced) | `● LIVE` | Green |
| SCROLL (browsing) | `↓ LIVE` (clickable) | Yellow, pulsing |
| SCROLL (at history boundary) | `↓ LIVE · history limit` | Yellow |
| Disconnected | `○ DISCONNECTED` | Red |

---

## 6. Implementation Strategy

### Phase 1: Server-Side Ring Buffer (relay-server changes)

**Estimated scope**: ~400 LOC in relay-server

1. **Add VT100 parser to Room**
   - Add `vt100` crate dependency to relay-server
   - Create `TerminalHistory` struct:
     ```rust
     pub struct TerminalHistory {
         parser: vt100::Parser,
         lines: VecDeque<HistoryLine>,
         capacity: usize,
         next_seq: u64,
         last_row_count: usize,
     }
     ```
   - Feed all incoming host binary frames into parser
   - After each feed, diff screen rows to detect new completed lines
   - Push completed lines to ring buffer

2. **Add HistoryRequest/Response handling to viewer relay loop**
   - Parse `HistoryRequest` from viewer text frames
   - Read requested range from `TerminalHistory`
   - Send `HistoryResponse` back to requesting viewer only (not broadcast)
   - Rate-limit: Max 10 history requests per second per viewer

3. **Send HistoryInfo on viewer join**
   - After AuthResult + Snapshot, send HistoryInfo
   - Periodically send HistoryInfo updates (every 30s or on significant change)

### Phase 2: Browser Viewer Scrollback (viewer.html changes)

**Estimated scope**: ~300 LOC in JavaScript

1. **Extend xterm.js configuration**
   - Set `scrollback: 50000` (client-side buffer for received content)
   - Track current scroll position vs live position

2. **Implement LIVE/SCROLL state machine**
   - Detect scroll events to transition LIVE → SCROLL
   - Add floating "↓ LIVE" indicator
   - Handle snap-to-live on End key or indicator click

3. **History fetch logic**
   - On scroll into unfetched territory, send `HistoryRequest`
   - Maintain client-side line cache (Map<seq, HistoryLine>)
   - Prefetch next chunk when approaching cache boundary
   - Insert fetched lines into xterm.js scrollback buffer

4. **Late-join enhancement**
   - On receiving `HistoryInfo`, auto-fetch last 200 lines
   - Prepend to terminal before snapshot renders

### Phase 3: Optimizations

1. **Compression**: gzip-compress `HistoryResponse.lines` for large payloads (>10KB)
2. **Adaptive chunk sizing**: Smaller chunks on slow connections
3. **Memory limits config**: Environment variable `AH_HISTORY_LINES=10000`
4. **Metrics**: Track buffer utilization, request frequency, memory usage per room

---

## 7. Dependencies

### Depends On
- Existing relay-server broadcast pipeline (binary frames)
- Existing Snapshot mechanism (used as sync point)
- `vt100` crate (already a dependency in pro/ client; needs adding to relay-server)

### Enables
- **SPEC-03: Presence & Cursor Tracking** — scroll position reporting requires the seq-number coordinate system defined here
- **Future: Session Recording** — ring buffer could be persisted to disk for session replay
- **Future: Search in History** — with structured lines, grep-style search becomes feasible

### New Dependencies
| Crate | Version | Purpose |
|-------|---------|---------|
| `vt100` | 0.15 | Terminal parsing on relay server |

---

## 8. Open Questions

### OQ-1: Line vs. Byte Addressing
**Question**: Should the ring buffer store **logical terminal lines** (post-VT100-parsing) or **raw byte chunks** with byte-offset addressing?

**Recommendation**: Logical lines. They're more useful for scrolling UX (scroll by line, not by byte), and the VT100 parser is already available. Raw bytes would require client-side re-parsing.

**Trade-off**: VT100 parsing adds CPU cost on relay server (~5μs per frame). For 100 concurrent rooms this is negligible.

### OQ-2: Ring Buffer Capacity Default
**Question**: What should the default ring buffer size be?

**Options**:
- 5,000 lines (~2.5MB per room assuming 500 byte avg line)
- 10,000 lines (~5MB per room) ← **recommended**
- 50,000 lines (~25MB per room)

**Recommendation**: 10,000 lines default, configurable via `AH_HISTORY_LINES` env var. At 100 concurrent rooms, this is ~500MB total — acceptable for a dedicated relay server.

### OQ-3: ANSI State at Line Boundaries
**Question**: How to handle ANSI state that spans across lines (e.g., a color set on line N that applies to line N+1)?

**Recommendation**: Store the "ANSI reset prefix" at the start of each line — the escape sequence that restores the terminal state to what it was at the start of that line. This makes each line independently renderable. The `vt100` crate tracks this state per-cell, so we can derive the prefix from the parser state when capturing each line.

### OQ-4: Binary Frame Ordering Guarantees
**Question**: Can we rely on binary frames arriving at the relay server in the same order they were generated?

**Answer**: Yes — WebSocket guarantees in-order delivery within a single connection, and there's exactly one host connection per room. Sequence numbers still provide an additional correctness guarantee.

### OQ-5: History for Multiple Panes
**Question**: If a host shares a tmux window with multiple panes, should each pane have independent history?

**Recommendation**: Defer to V3. Currently, pipe-pane captures the active pane's PTY output as a single stream. Multi-pane history would require per-pane tracking, which is a significant architecture change.

### OQ-6: Persistence Across Host Reconnection
**Question**: If the host disconnects and reconnects (within the 120s grace period), should the ring buffer be preserved?

**Recommendation**: Yes. The room object (including TerminalHistory) persists as long as the room exists. Host reconnection should resume appending to the existing buffer. This is already the behavior for `last_snapshot`.
