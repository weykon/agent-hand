# Multi-Viewer Sessions Management Design

**Date**: 2026-03-07
**Status**: Approved
**Author**: Claude (with user weykon)

## Overview

Enable users to connect to multiple remote sessions simultaneously as a viewer, manage these connections, and switch between them efficiently. This supports the use case where a user helps multiple people by viewing their terminal sessions concurrently.

## User Requirements

1. Connect to multiple remote sessions simultaneously (5+ concurrent connections)
2. View all connected sessions in the Sessions list
3. Quick switching between viewer sessions
4. Each connection maintains independent state (scroll position, permissions, statistics)
5. Graceful disconnect with options: disconnect+delete, disconnect-only, or cancel
6. Fix viewer display corruption issue (already addressed with buffer size reduction)

## Architecture: Separation of Metadata and Runtime State

### Core Principle

Separate lightweight metadata (for display) from heavy runtime state (for active viewing):

- **ViewerSessionInfo**: Lightweight metadata, serializable, persists across disconnects
- **ViewerState**: Heavy runtime state (WebSocket, terminal buffer, tasks), only for active connections

### Data Structures

```rust
/// Lightweight metadata for viewer sessions
#[cfg(feature = "pro")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewerSessionInfo {
    pub room_id: String,
    pub share_url: String,
    pub relay_url: String,
    pub viewer_token: String,
    pub host_name: Option<String>,
    pub session_name: String,
    pub permission: String,  // "ro" | "rw"
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub status: ViewerSessionStatus,
}

#[cfg(feature = "pro")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewerSessionStatus {
    Connecting,
    Connected,
    Disconnected { disconnected_at: Instant, reason: Option<String> },
    Reconnecting { attempt: u32 },
}

/// App structure changes
#[cfg(feature = "pro")]
pub struct App {
    // ... existing fields ...

    /// All viewer sessions (including disconnected)
    viewer_sessions: HashMap<String, ViewerSessionInfo>,

    /// Active viewer runtime states (Connected/Reconnecting only)
    viewer_states: HashMap<String, ViewerState>,

    /// Currently viewing room_id (when in ViewerMode)
    active_viewer_id: Option<String>,
}
```

## Component Design

### 1. Connection Management

**connect_viewer() changes**:
- Create ViewerSessionInfo and insert into `viewer_sessions`
- Create ViewerState and insert into `viewer_states`
- Set `active_viewer_id` to the new room_id
- Switch to ViewerMode

**disconnect_viewer() - new function**:
```rust
async fn disconnect_viewer(&mut self, room_id: &str, delete: bool) -> Result<()> {
    // 1. Get viewer_state and send close signal
    if let Some(vs) = self.viewer_states.remove(room_id) {
        let _ = vs.control_tx.send("__close__".to_string()).await;
        if let Some(handle) = vs.task_handle {
            handle.abort();
        }
    }

    // 2. Update ViewerSessionInfo status or remove
    if delete {
        self.viewer_sessions.remove(room_id);
    } else if let Some(info) = self.viewer_sessions.get_mut(room_id) {
        info.status = ViewerSessionStatus::Disconnected {
            disconnected_at: Instant::now(),
            reason: Some("User requested".to_string()),
        };
        info.last_seen = Instant::now();
    }

    // 3. Clear active_viewer_id if this was active
    if self.active_viewer_id.as_ref() == Some(&room_id.to_string()) {
        self.active_viewer_id = None;
        self.state = AppState::Normal;
    }

    Ok(())
}
```

**reconnect_viewer() - new function**:
```rust
async fn reconnect_viewer(&mut self, room_id: &str) -> Result<()> {
    let info = self.viewer_sessions.get(room_id)
        .ok_or_else(|| anyhow!("Session not found"))?;

    // Extract connection parameters
    let relay_url = info.relay_url.clone();
    let viewer_token = info.viewer_token.clone();

    // Update status to Reconnecting
    if let Some(info) = self.viewer_sessions.get_mut(room_id) {
        info.status = ViewerSessionStatus::Reconnecting { attempt: 1 };
    }

    // Call connect_viewer with existing room_id
    self.connect_viewer(&relay_url, room_id, &viewer_token).await
}
```

### 2. UI Rendering

**Sessions List Changes** (src/ui/render.rs):

```rust
// After rendering local sessions, add viewer sessions
let viewer_sessions: Vec<_> = app.viewer_sessions
    .values()
    .collect();

if !viewer_sessions.is_empty() {
    // Add separator
    items.push(ListItem::new("─────────────────────────"));

    // Add viewer sessions
    for (idx, info) in viewer_sessions.iter().enumerate() {
        let is_selected = app.active_panel_selected == (local_count + 1 + idx);
        let prefix = if is_selected { "> " } else { "  " };

        let status_icon = match info.status {
            ViewerSessionStatus::Connected => "👁",
            ViewerSessionStatus::Connecting => "⏳",
            ViewerSessionStatus::Reconnecting { .. } => "🔄",
            ViewerSessionStatus::Disconnected { .. } => "💤",
        };

        let permission_badge = match info.permission.as_str() {
            "rw" => " [RW]",
            _ => " [RO]",
        };

        let duration = format_duration(info.connected_at.elapsed());

        let line = format!(
            "{}{} Room {} ({}) {} • {}",
            prefix,
            status_icon,
            &info.room_id[..8],
            info.host_name.as_deref().unwrap_or("unknown"),
            permission_badge,
            duration
        );

        items.push(ListItem::new(line));
    }
}
```

### 3. Disconnect Confirmation Dialog

**New Dialog Type** (src/ui/dialogs.rs):

```rust
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct DisconnectViewerDialog {
    pub room_id: String,
    pub session_name: String,
    pub selected_option: usize,  // 0=disconnect+delete, 1=disconnect-only, 2=cancel
}
```

**Dialog Rendering**:
```
┌─ Disconnect Viewer Session ─────────────────────────────┐
│ Room: a3476376 (weykonkong)                             │
│ Session: agent-deck-rs                                  │
│                                                         │
│ > Disconnect and delete (remove from list)             │
│   Disconnect only (keep for reconnection)              │
│   Cancel                                                │
│                                                         │
│ Enter: Confirm  |  Up/Down: Select  |  Esc: Cancel     │
└─────────────────────────────────────────────────────────┘
```

### 4. Event Handling

**Key 'd' on viewer session**:
```rust
if key == KeyCode::Char('d') && is_viewer_session_selected {
    let room_id = get_selected_viewer_room_id();
    let session_name = app.viewer_sessions.get(&room_id)
        .map(|info| info.session_name.clone())
        .unwrap_or_default();

    self.dialog = Some(Dialog::DisconnectViewer(DisconnectViewerDialog {
        room_id,
        session_name,
        selected_option: 0,
    }));
    self.state = AppState::Dialog;
}
```

**Dialog handling**:
```rust
Dialog::DisconnectViewer(d) => match key {
    KeyCode::Up => d.selected_option = d.selected_option.saturating_sub(1),
    KeyCode::Down => d.selected_option = (d.selected_option + 1).min(2),
    KeyCode::Enter => {
        match d.selected_option {
            0 => {
                // Disconnect and delete
                self.disconnect_viewer(&d.room_id, true).await?;
            }
            1 => {
                // Disconnect only
                self.disconnect_viewer(&d.room_id, false).await?;
            }
            2 => {
                // Cancel - do nothing
            }
        }
        self.dialog = None;
        self.state = AppState::Normal;
    }
    KeyCode::Esc => {
        self.dialog = None;
        self.state = AppState::Normal;
    }
    _ => {}
}
```

## Implementation Plan

### Phase 1: Data Structures (30 min)
1. Add ViewerSessionInfo and ViewerSessionStatus to src/ui/mod.rs
2. Update App struct with new fields
3. Update App::new() to initialize new fields

### Phase 2: Connection Management (45 min)
1. Refactor connect_viewer() to create both info and state
2. Implement disconnect_viewer()
3. Implement reconnect_viewer()
4. Update existing viewer cleanup logic

### Phase 3: UI Rendering (30 min)
1. Update render_active_panel() to show viewer sessions
2. Add format_duration() helper
3. Update selection logic to handle viewer sessions

### Phase 4: Dialog (30 min)
1. Add DisconnectViewerDialog to dialogs.rs
2. Implement render_disconnect_viewer_dialog()
3. Add dialog handling in handle_dialog_key()

### Phase 5: Event Handling (30 min)
1. Update 'd' key handler for viewer sessions
2. Update Enter key handler to switch to viewer sessions
3. Update Ctrl+Q to return from ViewerMode

### Phase 6: Testing & Bug Fixes (30 min)
1. Test multi-session connection
2. Test disconnect/reconnect
3. Verify viewer display (buffer fix already applied)
4. Test dialog flow

**Total Estimated Time**: 3 hours

## Success Criteria

- [ ] Can connect to 5+ viewer sessions simultaneously
- [ ] All sessions appear in Sessions list with correct status
- [ ] Can switch between viewer sessions by selecting and pressing Enter
- [ ] Disconnect dialog works correctly with all three options
- [ ] Disconnected sessions can be reconnected
- [ ] Viewer display shows correctly (no corruption)
- [ ] Memory usage is reasonable (disconnected sessions don't hold buffers)

## Future Enhancements (Out of Scope)

- Quick switch hotkeys (1-9) in ViewerMode
- Session tagging/labeling
- Persistent session history across app restarts
- Split-screen viewing
- Session search/filter
