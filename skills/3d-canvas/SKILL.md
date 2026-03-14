# 3D Canvas — WebSocket Data Transport Skill

**Tier**: Max
**Port**: `3847` (default, configurable via `config.toml`)

Agent Hand exposes a WebSocket server that streams real-time session, group, relationship, and canvas data. You build the visualization — we provide the data.

## Quick Start

```javascript
const ws = new WebSocket('ws://127.0.0.1:3847');

ws.onopen = () => {
  // Subscribe to channels you want
  ws.send(JSON.stringify({
    type: 'subscribe',
    channels: ['sessions', 'canvas', 'groups', 'relationships']
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  switch (msg.type) {
    case 'welcome':  console.log('Connected, version:', msg.version); break;
    case 'push':     console.log('Update on', msg.channel, msg.data); break;
    case 'response': console.log('Response:', msg.id, msg.data); break;
    case 'error':    console.error(msg.message); break;
  }
};
```

## Wire Protocol

### Client -> Server

All messages are JSON with a `type` field.

#### Subscribe
```json
{ "type": "subscribe", "channels": ["sessions", "canvas"] }
```
Start receiving push updates for the listed channels.

#### Unsubscribe
```json
{ "type": "unsubscribe", "channels": ["canvas"] }
```
Stop receiving push updates for the listed channels.

#### Request (query)
```json
{ "type": "request", "id": "req-1", "op": { "op": "list_sessions" } }
```
Send a read-only query. The server responds with a `response` message matching the `id`.

Available operations:
| Operation | Description |
|-----------|-------------|
| `{ "op": "list_sessions" }` | All sessions with status, group, tags |
| `{ "op": "list_groups" }` | All groups with session counts |
| `{ "op": "status" }` | Aggregate status counts (running, idle, etc.) |
| `{ "op": "query_canvas", "what": "state" }` | Full canvas graph (nodes + edges + viewport) |
| `{ "op": "query_canvas", "what": "nodes" }` | Canvas nodes only |
| `{ "op": "query_canvas", "what": "edges" }` | Canvas edges only |

### Server -> Client

#### Welcome (on connect)
```json
{
  "type": "welcome",
  "channels": ["canvas", "sessions", "groups", "relationships"],
  "version": "0.3.8"
}
```

#### Push (real-time update)
```json
{
  "type": "push",
  "channel": "sessions",
  "data": [
    { "id": "abc123", "title": "my-project", "status": "running", "group_path": "default", ... }
  ]
}
```
Sent when subscribed channel data changes. Broadcasts happen on each tick (~250ms) with hash-based change detection.

#### Response (to request)
```json
{ "type": "response", "id": "req-1", "data": [...] }
```

#### Error
```json
{ "type": "error", "message": "invalid message: ..." }
```

## Channel Data Shapes

### `sessions`
Array of session objects:
```typescript
interface Session {
  id: string;
  title: string;
  project_path: string;
  group_path: string;
  status: "idle" | "running" | "waiting" | "error" | "ready";
  label: string;
  label_color: string;
  tags: string[];
  command?: string;
  tool?: string;
}
```

### `groups`
Array of group objects:
```typescript
interface Group {
  path: string;       // e.g. "default", "work/backend"
  name: string;       // e.g. "backend"
  session_count: number;
}
```

### `relationships`
Array of relationship objects:
```typescript
interface Relationship {
  id: string;
  session_a: string;
  session_b: string;
  relation_type: "parent_child" | "peer" | "dependency" | "collaboration" | "custom";
  label?: string;
  bidirectional: boolean;
}
```

### `canvas`
Canvas state object:
```typescript
interface CanvasState {
  nodes: Array<{
    id: string;
    label: string;
    kind: "start" | "end" | "process" | "decision" | "note";
    pos: [number, number];
    content?: string;
  }>;
  edges: Array<{
    source: string;
    target: string;
    label?: string;
  }>;
  cursor: [number, number];
  viewport: { x: number; y: number };
}
```

## Configuration

In `~/.agent-hand/config.toml`:
```toml
[ws]
enabled = true       # default: true
port = 3847          # default: 3847
bind = "127.0.0.1"  # default: "127.0.0.1"
```

Set `bind = "0.0.0.0"` to allow connections from other machines (e.g. a phone on the same network).

## Building a Visualization

The `starter.html` file in this directory is a self-contained Three.js example that renders sessions as a 3D force-directed graph. Use it as a reference or starting point.

Key patterns:
1. Connect on page load, subscribe to all channels
2. On `push` for `sessions`, rebuild node geometry
3. On `push` for `relationships`, rebuild edge geometry
4. On `push` for `canvas`, overlay canvas nodes/edges
5. Use `request` for initial data load before first push arrives
