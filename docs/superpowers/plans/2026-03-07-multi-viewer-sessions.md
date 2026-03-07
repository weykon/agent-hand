# Multi-Viewer Sessions Implementation Plan

> **For Claude:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable connecting to 5+ remote viewer sessions simultaneously with session switching, disconnect/reconnect, and state management

**Architecture:** HashMap-based session storage with separation of metadata (ViewerSessionInfo) from runtime state (ViewerState). Status state machine tracks connection lifecycle. Modal dialogs for user confirmation on destructive actions.

**Tech Stack:** Rust, tokio async, ratatui TUI, WebSocket relay, vt100 parser

---

## Task 1: Data Structures

**Files:**
- Modify: `src/ui/app.rs` (add ViewerSessionInfo, ViewerSessionStatus structs)
- Modify: `src/ui/dialogs.rs` (add DisconnectViewerDialog)

- [ ] **Step 1: Add ViewerSessionInfo struct to app.rs**

Add after the existing structs around line 100:

```rust
#[derive(Clone, Debug)]
pub struct ViewerSessionInfo {
    pub room_id: String,
    pub relay_url: String,
    pub connected_at: std::time::SystemTime,
    pub status: ViewerSessionStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewerSessionStatus {
    Connecting,
    Connected,
    Disconnected,
    Reconnecting,
}
```

- [ ] **Step 2: Add viewer_sessions HashMap to App struct**

In `src/ui/app.rs`, find the App struct (around line 150) and add:

```rust
pub struct App {
    // ... existing fields ...

    /// Metadata for all viewer sessions (persists across disconnects)
    pub viewer_sessions: HashMap<String, ViewerSessionInfo>,
}
```

- [ ] **Step 3: Initialize viewer_sessions in App::new()**

In `src/ui/app.rs`, find `App::new()` (around line 300) and add:

```rust
viewer_sessions: HashMap::new(),
```

- [ ] **Step 4: Add DisconnectViewerDialog to dialogs.rs**

In `src/ui/dialogs.rs`, add after existing dialog structs (around line 400):

```rust
#[derive(Clone)]
pub struct DisconnectViewerDialog {
    pub room_id: String,
    pub relay_url: String,
    pub selected_option: usize, // 0=disconnect only, 1=disconnect+delete, 2=cancel
}

impl DisconnectViewerDialog {
    pub fn new(room_id: String, relay_url: String) -> Self {
        Self {
            room_id,
            relay_url,
            selected_option: 0,
        }
    }
}
```

- [ ] **Step 5: Add DisconnectViewerDialog variant to Dialog enum**

In `src/ui/dialogs.rs`, find the Dialog enum (around line 50) and add:

```rust
pub enum Dialog {
    // ... existing variants ...
    DisconnectViewer(DisconnectViewerDialog),
}
```

- [ ] **Step 6: Verify compilation**

```bash
cargo check
```

Expected: No errors, new structs compile successfully

- [ ] **Step 7: Commit data structures**

```bash
git add src/ui/app.rs src/ui/dialogs.rs
git commit -m "feat(viewer): add multi-session data structures

- Add ViewerSessionInfo and ViewerSessionStatus
- Add viewer_sessions HashMap to App
- Add DisconnectViewerDialog for user confirmation"
```

---

## Task 2: Connection Management - Store Session Metadata

**Files:**
- Modify: `src/ui/app.rs:5000-5200` (connect_viewer function)

- [ ] **Step 1: Store session info after successful connection**

In `src/ui/app.rs`, find the `connect_viewer` function (around line 5033). After the WebSocket connection succeeds and before spawning the read task, add:

```rust
// After: let (ws_stream, _) = connect_result??;
// Before: let (mut write, read) = ws_stream.split();

// Store session metadata
let session_info = ViewerSessionInfo {
    room_id: room_id.clone(),
    relay_url: relay_url.clone(),
    connected_at: std::time::SystemTime::now(),
    status: ViewerSessionStatus::Connected,
};
self.viewer_sessions.insert(room_id.clone(), session_info);
```

- [ ] **Step 2: Update status to Connecting before connection attempt**

Before the `tokio::time::timeout` call, add:

```rust
// Set status to Connecting
let session_info = ViewerSessionInfo {
    room_id: room_id.clone(),
    relay_url: relay_url.clone(),
    connected_at: std::time::SystemTime::now(),
    status: ViewerSessionStatus::Connecting,
};
self.viewer_sessions.insert(room_id.clone(), session_info);
```

- [ ] **Step 3: Update status to Disconnected on connection failure**

In the error handling block (around line 5100), add:

```rust
// On error, update status
if let Some(session) = self.viewer_sessions.get_mut(&room_id) {
    session.status = ViewerSessionStatus::Disconnected;
}
```

- [ ] **Step 4: Test connection and verify session storage**

```bash
cargo build --release
./target/release/agent-hand

# In TUI:
# 1. Start sharing from one terminal
# 2. Join from another terminal
# 3. Check that connection succeeds
```

Expected: Connection works, no crashes

- [ ] **Step 5: Commit connection metadata storage**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): store session metadata on connect

- Store ViewerSessionInfo in viewer_sessions HashMap
- Track Connecting → Connected status transitions
- Update to Disconnected on connection failure"
```

---

## Task 3: Disconnect Function

**Files:**
- Modify: `src/ui/app.rs` (add disconnect_viewer function)

- [ ] **Step 1: Add disconnect_viewer function**

In `src/ui/app.rs`, add after the `connect_viewer` function (around line 5200):

```rust
pub async fn disconnect_viewer(&mut self, room_id: &str, delete_session: bool) {
    // Update status to Disconnected
    if let Some(session) = self.viewer_sessions.get_mut(room_id) {
        session.status = ViewerSessionStatus::Disconnected;
    }

    // If currently viewing this session, exit viewer mode
    if self.mode == AppMode::Viewer {
        if let Some(ref viewer_state) = self.viewer_state {
            if viewer_state.room_id == room_id {
                self.mode = AppMode::Dashboard;
                self.viewer_state = None;
            }
        }
    }

    // Close WebSocket connection if exists
    // (The read task will detect closure and clean up)

    // Delete session metadata if requested
    if delete_session {
        self.viewer_sessions.remove(room_id);
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Commit disconnect function**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): add disconnect_viewer function

- Update session status to Disconnected
- Exit viewer mode if viewing disconnected session
- Optionally delete session metadata"
```

---

## Task 4: Reconnect Function

**Files:**
- Modify: `src/ui/app.rs` (add reconnect_viewer function)

- [ ] **Step 1: Add reconnect_viewer function**

In `src/ui/app.rs`, add after `disconnect_viewer`:

```rust
pub async fn reconnect_viewer(&mut self, room_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get session info
    let session_info = self.viewer_sessions.get(room_id)
        .ok_or("Session not found")?
        .clone();

    // Update status to Reconnecting
    if let Some(session) = self.viewer_sessions.get_mut(room_id) {
        session.status = ViewerSessionStatus::Reconnecting;
    }

    // Reuse connect_viewer logic
    let relay_url = session_info.relay_url.clone();
    let room_id = session_info.room_id.clone();

    // Call connect_viewer (which will update status to Connected on success)
    self.connect_viewer(&relay_url, &room_id).await
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Commit reconnect function**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): add reconnect_viewer function

- Retrieve session info from viewer_sessions
- Update status to Reconnecting
- Reuse connect_viewer for actual connection"
```

---

## Task 5: UI Rendering - Sessions List

**Files:**
- Modify: `src/ui/render.rs:2500-2700` (render_active_panel function)

- [ ] **Step 1: Add viewer sessions section to Dashboard**

In `src/ui/render.rs`, find `render_active_panel` function (around line 2500). After rendering the instances list, add a new section for viewer sessions:

```rust
// After rendering instances, add viewer sessions section
if !app.viewer_sessions.is_empty() {
    let viewer_sessions_block = Block::default()
        .borders(Borders::ALL)
        .title(" Remote Viewer Sessions ");

    let viewer_items: Vec<ListItem> = app.viewer_sessions
        .iter()
        .map(|(room_id, info)| {
            let status_icon = match info.status {
                ViewerSessionStatus::Connecting => "⟳",
                ViewerSessionStatus::Connected => "●",
                ViewerSessionStatus::Disconnected => "○",
                ViewerSessionStatus::Reconnecting => "⟳",
            };

            let status_color = match info.status {
                ViewerSessionStatus::Connected => Color::Green,
                ViewerSessionStatus::Connecting | ViewerSessionStatus::Reconnecting => Color::Yellow,
                ViewerSessionStatus::Disconnected => Color::Red,
            };

            let line = Line::from(vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::raw(room_id.clone()),
                Span::raw(" - "),
                Span::styled(info.relay_url.clone(), Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let viewer_list = List::new(viewer_items)
        .block(viewer_sessions_block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    // Render in a new chunk below instances
    // (You'll need to adjust the layout constraints)
}
```

- [ ] **Step 2: Adjust layout constraints to make room**

In the same function, find the layout constraints (around line 2550) and modify:

```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),      // Header
        Constraint::Min(5),          // Instances (reduced from Min(10))
        Constraint::Length(3 + app.viewer_sessions.len() as u16), // Viewer sessions
        Constraint::Length(3),      // Footer
    ])
    .split(area);
```

- [ ] **Step 3: Test rendering with mock data**

```bash
cargo build --release
./target/release/agent-hand

# Manually add a test session in code temporarily:
# In App::new(), add:
# self.viewer_sessions.insert("test-room".to_string(), ViewerSessionInfo { ... });
```

Expected: Viewer sessions section appears in Dashboard

- [ ] **Step 4: Remove test data and commit**

```bash
git add src/ui/render.rs
git commit -m "feat(viewer): render viewer sessions list in Dashboard

- Show all viewer sessions with status icons
- Color-code by status (green=connected, yellow=connecting, red=disconnected)
- Display room_id and relay_url"
```

---

## Task 6: DisconnectViewerDialog Rendering

**Files:**
- Modify: `src/ui/render.rs:2800-3000` (render_dialogs function)

- [ ] **Step 1: Add DisconnectViewerDialog rendering**

In `src/ui/render.rs`, find `render_dialogs` function (around line 2800). Add a new match arm for DisconnectViewer:

```rust
Dialog::DisconnectViewer(d) => {
    let dialog_block = Block::default()
        .borders(Borders::ALL)
        .title(" Disconnect Viewer Session ")
        .border_style(Style::default().fg(Color::Yellow));

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Room: "),
            Span::styled(&d.room_id, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Relay: "),
            Span::styled(&d.relay_url, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from("What would you like to do?"),
        Line::from(""),
        Line::from(vec![
            if d.selected_option == 0 {
                Span::styled("> Disconnect (keep session)", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("  Disconnect (keep session)")
            },
        ]),
        Line::from(vec![
            if d.selected_option == 1 {
                Span::styled("> Disconnect and delete session", Style::default().fg(Color::Red))
            } else {
                Span::raw("  Disconnect and delete session")
            },
        ]),
        Line::from(vec![
            if d.selected_option == 2 {
                Span::styled("> Cancel", Style::default().fg(Color::Green))
            } else {
                Span::raw("  Cancel")
            },
        ]),
        Line::from(""),
        Line::from("Use ↑/↓ to select, Enter to confirm"),
    ];

    let paragraph = Paragraph::new(text)
        .block(dialog_block)
        .wrap(Wrap { trim: true });

    let area = centered_rect(60, 50, f.size());
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Commit dialog rendering**

```bash
git add src/ui/render.rs
git commit -m "feat(viewer): render DisconnectViewerDialog

- Show room_id and relay_url
- Three options: disconnect only, disconnect+delete, cancel
- Highlight selected option"
```

---

## Task 7: Event Handling - 'd' Key in Dashboard

**Files:**
- Modify: `src/ui/app.rs:3500-3700` (handle_key_event for Dashboard mode)

- [ ] **Step 1: Add 'd' key handler for viewer sessions**

In `src/ui/app.rs`, find the `handle_key_event` function for Dashboard mode (around line 3500). Add handling for 'd' key when a viewer session is selected:

```rust
KeyCode::Char('d') => {
    // Check if we're in viewer sessions selection
    // (You'll need to track which list is focused: instances or viewer_sessions)
    // For now, assume we have a way to determine selected viewer session

    if let Some(selected_room_id) = self.get_selected_viewer_session() {
        if let Some(session_info) = self.viewer_sessions.get(&selected_room_id) {
            let dialog = DisconnectViewerDialog::new(
                session_info.room_id.clone(),
                session_info.relay_url.clone(),
            );
            self.dialog = Some(Dialog::DisconnectViewer(dialog));
        }
    }
}
```

- [ ] **Step 2: Add helper function get_selected_viewer_session**

In `src/ui/app.rs`, add:

```rust
fn get_selected_viewer_session(&self) -> Option<String> {
    // This is a placeholder - you'll need to implement proper selection tracking
    // For now, return the first session if any exist
    self.viewer_sessions.keys().next().cloned()
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 4: Commit 'd' key handler**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): add 'd' key to open disconnect dialog

- Open DisconnectViewerDialog when 'd' pressed on viewer session
- Add placeholder get_selected_viewer_session helper"
```

---

## Task 8: Event Handling - DisconnectViewerDialog

**Files:**
- Modify: `src/ui/app.rs:4000-4200` (handle_key_event for dialog mode)

- [ ] **Step 1: Add DisconnectViewerDialog key handling**

In `src/ui/app.rs`, find dialog key handling (around line 4000). Add a new match arm:

```rust
Dialog::DisconnectViewer(ref mut d) => {
    match key.code {
        KeyCode::Up => {
            if d.selected_option > 0 {
                d.selected_option -= 1;
            }
        }
        KeyCode::Down => {
            if d.selected_option < 2 {
                d.selected_option += 1;
            }
        }
        KeyCode::Enter => {
            let room_id = d.room_id.clone();
            let option = d.selected_option;
            self.dialog = None;

            match option {
                0 => {
                    // Disconnect only
                    self.disconnect_viewer(&room_id, false).await;
                }
                1 => {
                    // Disconnect and delete
                    self.disconnect_viewer(&room_id, true).await;
                }
                2 => {
                    // Cancel - do nothing
                }
                _ => {}
            }
        }
        KeyCode::Esc => {
            self.dialog = None;
        }
        _ => {}
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Commit dialog event handling**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): handle DisconnectViewerDialog events

- Up/Down to navigate options
- Enter to confirm selection
- Esc to cancel
- Call disconnect_viewer with appropriate delete flag"
```

---

## Task 9: Event Handling - Enter to Switch Sessions

**Files:**
- Modify: `src/ui/app.rs:3500-3700` (handle_key_event for Dashboard mode)

- [ ] **Step 1: Add Enter key handler for viewer sessions**

In `src/ui/app.rs`, in Dashboard mode key handling, add:

```rust
KeyCode::Enter => {
    if let Some(selected_room_id) = self.get_selected_viewer_session() {
        if let Some(session_info) = self.viewer_sessions.get(&selected_room_id) {
            match session_info.status {
                ViewerSessionStatus::Connected => {
                    // Switch to this viewer session
                    // (Viewer state should already exist from connection)
                    self.mode = AppMode::Viewer;
                }
                ViewerSessionStatus::Disconnected => {
                    // Attempt reconnect
                    if let Err(e) = self.reconnect_viewer(&selected_room_id).await {
                        // Show error dialog
                        eprintln!("Reconnect failed: {}", e);
                    }
                }
                _ => {
                    // Connecting or Reconnecting - do nothing
                }
            }
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Commit Enter key handler**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): add Enter to switch/reconnect sessions

- Enter on Connected session switches to viewer mode
- Enter on Disconnected session attempts reconnect
- No action on Connecting/Reconnecting"
```

---

## Task 10: Event Handling - Ctrl+Q in Viewer Mode

**Files:**
- Modify: `src/ui/app.rs:3800-4000` (handle_key_event for Viewer mode)

- [ ] **Step 1: Update Ctrl+Q to return to Dashboard**

In `src/ui/app.rs`, find Viewer mode key handling (around line 3800). Modify the Ctrl+Q handler:

```rust
KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    // Return to Dashboard (don't disconnect)
    self.mode = AppMode::Dashboard;
    // Keep viewer_state and session info intact
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 3: Test Ctrl+Q behavior**

```bash
cargo build --release
./target/release/agent-hand

# 1. Connect to a viewer session
# 2. Press Ctrl+Q
# 3. Verify you return to Dashboard
# 4. Verify session still shows as Connected
# 5. Press Enter on the session
# 6. Verify you return to viewer mode
```

Expected: Ctrl+Q returns to Dashboard without disconnecting

- [ ] **Step 4: Commit Ctrl+Q update**

```bash
git add src/ui/app.rs
git commit -m "feat(viewer): Ctrl+Q returns to Dashboard without disconnect

- Keep viewer_state and session info intact
- Allow returning to viewer mode via Enter"
```

---

## Task 11: Integration Testing

**Files:**
- Test: Multi-session connection and switching

- [ ] **Step 1: Test connecting to multiple sessions**

```bash
# Terminal 1: Start first host
./target/release/agent-hand
# Press 's' to share

# Terminal 2: Start second host
./target/release/agent-hand
# Press 's' to share

# Terminal 3: Join both sessions
./target/release/agent-hand
# Press 'j' to join first session
# Press Ctrl+Q to return to Dashboard
# Press 'j' to join second session
# Verify both sessions appear in Dashboard
```

Expected: Both sessions visible in Dashboard with status icons

- [ ] **Step 2: Test session switching**

```bash
# In Terminal 3 (viewer):
# Press Ctrl+Q to return to Dashboard
# Use arrow keys to select first session
# Press Enter
# Verify you're viewing first session
# Press Ctrl+Q
# Select second session
# Press Enter
# Verify you're viewing second session
```

Expected: Can switch between sessions without reconnecting

- [ ] **Step 3: Test disconnect without delete**

```bash
# In Terminal 3:
# Press Ctrl+Q to return to Dashboard
# Select a session
# Press 'd'
# Select "Disconnect (keep session)"
# Press Enter
# Verify session shows as Disconnected (○ red icon)
# Press Enter on the session
# Verify reconnection attempt
```

Expected: Session disconnects but remains in list, can reconnect

- [ ] **Step 4: Test disconnect with delete**

```bash
# In Terminal 3:
# Press Ctrl+Q to return to Dashboard
# Select a session
# Press 'd'
# Select "Disconnect and delete session"
# Press Enter
# Verify session disappears from list
```

Expected: Session removed from Dashboard

- [ ] **Step 5: Test viewer display quality**

```bash
# In Terminal 1 (host):
# Run some commands: ls, cat file, etc.

# In Terminal 3 (viewer):
# Verify text displays correctly
# Verify no garbled characters
# Verify scrolling works
```

Expected: Clean display, no corruption

- [ ] **Step 6: Document test results**

Create `docs/superpowers/plans/2026-03-07-multi-viewer-testing.md`:

```markdown
# Multi-Viewer Sessions Testing Results

## Test Date: [DATE]

### Multi-Session Connection
- [ ] Connected to 2+ sessions simultaneously
- [ ] All sessions visible in Dashboard
- [ ] Status icons correct

### Session Switching
- [ ] Ctrl+Q returns to Dashboard
- [ ] Enter switches to selected session
- [ ] No reconnection required for active sessions

### Disconnect Operations
- [ ] Disconnect without delete keeps session
- [ ] Can reconnect to disconnected session
- [ ] Disconnect with delete removes session

### Display Quality
- [ ] No garbled text
- [ ] Scrolling works correctly
- [ ] Status updates in real-time

## Issues Found
[List any issues discovered during testing]

## Performance Notes
[Any performance observations]
```

- [ ] **Step 7: Final commit**

```bash
git add docs/superpowers/plans/2026-03-07-multi-viewer-testing.md
git commit -m "test(viewer): document multi-session testing results"
```

---

## Completion Checklist

- [ ] All 11 tasks completed
- [ ] All tests passing
- [ ] No compilation errors
- [ ] Documentation updated
- [ ] All commits pushed

## Known Limitations

1. **Selection Tracking**: The `get_selected_viewer_session()` function is a placeholder. Full implementation requires tracking which UI list (instances vs viewer_sessions) is focused and which item is selected.

2. **WebSocket Cleanup**: The disconnect function doesn't explicitly close WebSocket connections - it relies on the read task detecting closure. May need explicit close mechanism.

3. **Reconnection Logic**: Reconnection reuses `connect_viewer`, which may create duplicate viewer_state entries. May need cleanup logic.

4. **UI Layout**: The viewer sessions section uses fixed height based on session count. May need scrolling for many sessions.

## Future Enhancements

1. Add session nicknames/labels
2. Show connection duration
3. Add bandwidth usage stats
4. Support session grouping/folders
5. Add session search/filter
6. Persist sessions across app restarts
