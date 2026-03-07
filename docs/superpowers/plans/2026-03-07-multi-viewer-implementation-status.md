# Multi-Viewer Sessions Implementation Status

## Completed (2026-03-07)

### Core Data Structures ✅
- Added `ViewerSessionInfo` struct with room_id, relay_url, viewer_token, connected_at, status
- Added `ViewerSessionStatus` enum: Connecting, Connected, Disconnected, Reconnecting
- Added `viewer_sessions: HashMap<String, ViewerSessionInfo>` to App struct
- Added `room_id` field to ViewerState
- Added `DisconnectViewerDialog` struct and Dialog enum variant

### Connection Management ✅
- Modified `connect_viewer()` to store session metadata on connection
- Status transitions: Connecting → Connected → Disconnected
- Added `disconnect_viewer_session(room_id, delete_session)` function
- Added `reconnect_viewer(room_id)` function that reuses connect_viewer logic
- Updated `disconnect_viewer()` to update session status

### Dialog Event Handling ✅
- Implemented DisconnectViewerDialog key handling (Up/Down/Enter/Esc)
- Three options: disconnect only, disconnect+delete, cancel
- Integrated with disconnect_viewer_session function

## Remaining Tasks

### UI Rendering (High Priority)
- **Task 5**: Render viewer sessions list in Dashboard
  - Show all sessions with status icons (●=connected, ○=disconnected, ⟳=connecting/reconnecting)
  - Color-code by status (green/red/yellow)
  - Display room_id and relay_url

- **Task 6**: Render DisconnectViewerDialog
  - Show room_id and relay_url
  - Three options with selection highlight
  - Instructions at bottom

### Event Handling (High Priority)
- **Task 7**: Handle 'd' key in Dashboard
  - Open DisconnectViewerDialog when 'd' pressed on selected viewer session
  - Need to implement viewer session selection tracking

- **Task 9**: Handle Enter key for session switching
  - Enter on Connected session → switch to viewer mode
  - Enter on Disconnected session → attempt reconnect

- **Task 10**: Update Ctrl+Q behavior in Viewer mode
  - Return to Dashboard without disconnecting
  - Keep viewer_state and session info intact

### Testing (Medium Priority)
- **Task 11**: Integration testing
  - Multi-session connection
  - Session switching
  - Disconnect/reconnect
  - Display quality verification

## Technical Notes

### Session Selection
Currently missing: tracking which viewer session is selected in Dashboard. Need to add:
- `selected_viewer_session_index: usize` to App struct
- Navigation keys (Up/Down) to change selection
- Visual highlight for selected session

### Viewer State Management
- ViewerState contains runtime connection state (WebSocket, buffers, etc.)
- ViewerSessionInfo contains persistent metadata
- Separation allows disconnecting without losing session info

### Reconnection
- viewer_token is now stored in ViewerSessionInfo
- reconnect_viewer() can reuse connect_viewer() with stored credentials
- Status properly tracks Reconnecting state

## Commits

1. `f919071` - feat(viewer): add multi-session data structures
2. `1de98db` - feat(viewer): store session metadata on connect
3. `085cd1d` - feat(viewer): add reconnect_viewer function

## Next Steps

1. Implement viewer session selection in Dashboard (prerequisite for Task 7, 9)
2. Add UI rendering for viewer sessions list (Task 5)
3. Add DisconnectViewerDialog rendering (Task 6)
4. Wire up keyboard events (Tasks 7, 9, 10)
5. Test end-to-end functionality (Task 11)
