---
name: canvas-ops
description: Operate the agent-hand canvas workflow editor via agent-hand-bridge
---

# Canvas Operations

Control the agent-hand canvas workflow editor programmatically. The canvas is a
directed graph of nodes and edges rendered in the TUI.

**Prerequisite**: agent-hand must be running with the canvas visible (Pro feature).
Check with `agent-hand-bridge ping`.

## Binary

The bridge binary is **`agent-hand-bridge`** (not `agent-hand-hook`).
It communicates over a Unix domain socket at `~/.agent-hand/canvas.sock`.

## Quick Reference

```bash
# Send a canvas operation (inline JSON)
agent-hand-bridge canvas '{"op":"add_node","id":"n1","label":"My Task"}'

# Read operation JSON from stdin
echo '{"op":"add_node","id":"n1","label":"My Task"}' | agent-hand-bridge canvas -

# Query canvas state (shortcut)
agent-hand-bridge query nodes
agent-hand-bridge query edges
agent-hand-bridge query state
agent-hand-bridge query selected

# Batch operations from file
agent-hand-bridge canvas --batch ops.json
```

## CanvasOp JSON Format

All operations use `serde(tag = "op", rename_all = "snake_case")` tagging.
The `"op"` field determines the operation type (snake_case).

### add_node

Add a node to the canvas.

```json
{
  "op": "add_node",
  "id": "unique-id",
  "label": "Display Label",
  "kind": "Process",
  "pos": [5, 3],
  "content": "Optional multi-line\ncontent block"
}
```

| Field     | Type             | Required | Default     | Description                    |
|-----------|------------------|----------|-------------|--------------------------------|
| `id`      | string           | yes      |             | Unique node identifier         |
| `label`   | string           | yes      |             | Display text inside the node   |
| `kind`    | NodeKind         | no       | `"Process"` | Visual style (see below)       |
| `pos`     | `[col, row]`     | no       | auto-placed | Position as `[u16, u16]`       |
| `content` | string or null   | no       | null        | Multi-line content block       |

### remove_node

```json
{"op": "remove_node", "id": "n1"}
```

### update_node

Update properties of an existing node. Only provided fields are changed.

```json
{
  "op": "update_node",
  "id": "n1",
  "label": "New Label",
  "kind": "Decision",
  "pos": [10, 5],
  "content": "Updated content"
}
```

All fields except `id` are optional.

### add_edge

```json
{
  "op": "add_edge",
  "from": "n1",
  "to": "n2",
  "label": "depends on",
  "relationship_id": "optional-rel-id"
}
```

| Field             | Type           | Required | Description                      |
|-------------------|----------------|----------|----------------------------------|
| `from`            | string         | yes      | Source node ID                   |
| `to`              | string         | yes      | Target node ID                   |
| `label`           | string or null | no       | Edge label text                  |
| `relationship_id` | string or null | no       | Link to relationship system      |

### update_edge

```json
{"op": "update_edge", "from": "n1", "to": "n2", "label": "new label"}
```

### remove_edge

```json
{"op": "remove_edge", "from": "n1", "to": "n2"}
```

### layout

Trigger automatic layout of all nodes.

```json
{"op": "layout", "direction": "TopDown"}
```

| Direction     | Description               |
|---------------|---------------------------|
| `"TopDown"`   | Vertical stack (default)  |
| `"LeftRight"` | Horizontal row            |

### query

Query the current canvas state. Returns structured JSON. Supports optional filters.

```json
{"op": "query", "what": "nodes"}
{"op": "query", "what": "nodes", "kind": "Decision"}
{"op": "query", "what": "nodes", "label_contains": "API"}
{"op": "query", "what": "nodes", "id": "session:backend"}
{"op": "query", "what": "edges", "label_contains": "depends"}
```

| `what`       | Returns                                     |
|--------------|---------------------------------------------|
| `"nodes"`    | `{"type":"node_list","nodes":[...]}`         |
| `"edges"`    | `{"type":"edge_list","edges":[...]}`         |
| `"state"`    | `{"type":"state","json":{...}}`              |
| `"selected"` | Currently selected nodes                     |

**Filter fields** (all optional, applied to `nodes` and `edges`):

| Field             | Applies to | Description                              |
|-------------------|------------|------------------------------------------|
| `kind`            | nodes      | Filter by NodeKind (Start/End/Process/Decision/Note) |
| `label_contains`  | nodes, edges | Case-insensitive label substring match |
| `id`              | nodes      | Exact node ID match                      |

**CLI shortcuts**:

```bash
agent-hand-bridge query nodes --kind Decision
agent-hand-bridge query nodes --label "step"
agent-hand-bridge query nodes --id "session:abc123"
agent-hand-bridge query edges --label "depends"
```

### batch

Send multiple operations atomically.

```json
{
  "op": "batch",
  "ops": [
    {"op": "add_node", "id": "a", "label": "Step A"},
    {"op": "add_node", "id": "b", "label": "Step B"},
    {"op": "add_edge", "from": "a", "to": "b", "label": "then"}
  ]
}
```

### undo / redo

```json
{"op": "undo"}
{"op": "redo"}
```

## NodeKind Values

NodeKind uses PascalCase (no rename_all on this enum):

| Value        | Indicator | Border  | Color     | Use Case            |
|--------------|-----------|---------|-----------|---------------------|
| `"Start"`    | `>` arrow | Rounded | Green     | Entry points        |
| `"End"`      | square    | Rounded | Red       | Exit points         |
| `"Process"`  | (none)    | Plain   | Cyan      | Tasks, steps        |
| `"Decision"` | diamond   | Double  | Yellow    | Branching logic     |
| `"Note"`     | `#`       | Plain   | DarkGray  | Annotations         |

## Response Format

Responses use `serde(tag = "type", rename_all = "snake_case")`:

```json
{"type": "ok", "message": "node added"}
{"type": "node_list", "nodes": [{"id": "n1", "label": "Task", "kind": "Process", "pos": [2, 2]}]}
{"type": "edge_list", "edges": [{"from": "n1", "to": "n2", "label": "depends"}]}
{"type": "state", "json": {...}}
{"type": "error", "message": "node not found: n99"}
```

## Common Workflows

### Create a simple flowchart

```bash
# Create nodes
agent-hand-bridge canvas '{"op":"add_node","id":"start","label":"Begin","kind":"Start"}'
agent-hand-bridge canvas '{"op":"add_node","id":"step1","label":"Process Data","kind":"Process"}'
agent-hand-bridge canvas '{"op":"add_node","id":"check","label":"Valid?","kind":"Decision"}'
agent-hand-bridge canvas '{"op":"add_node","id":"done","label":"Complete","kind":"End"}'

# Connect them
agent-hand-bridge canvas '{"op":"add_edge","from":"start","to":"step1","label":"begin"}'
agent-hand-bridge canvas '{"op":"add_edge","from":"step1","to":"check"}'
agent-hand-bridge canvas '{"op":"add_edge","from":"check","to":"done","label":"yes"}'

# Auto-layout
agent-hand-bridge canvas '{"op":"layout","direction":"TopDown"}'
```

### Batch: build a pipeline in one call

```bash
cat <<'EOF' > /tmp/pipeline.json
[
  {"op": "add_node", "id": "fetch", "label": "Fetch Data", "kind": "Start"},
  {"op": "add_node", "id": "parse", "label": "Parse JSON", "kind": "Process"},
  {"op": "add_node", "id": "validate", "label": "Schema OK?", "kind": "Decision"},
  {"op": "add_node", "id": "store", "label": "Store Result", "kind": "Process"},
  {"op": "add_node", "id": "fail", "label": "Log Error", "kind": "End"},
  {"op": "add_edge", "from": "fetch", "to": "parse"},
  {"op": "add_edge", "from": "parse", "to": "validate"},
  {"op": "add_edge", "from": "validate", "to": "store", "label": "valid"},
  {"op": "add_edge", "from": "validate", "to": "fail", "label": "invalid"},
  {"op": "layout", "direction": "TopDown"}
]
EOF
agent-hand-bridge canvas --batch /tmp/pipeline.json
```

### Add session nodes (convention: prefix with `session:`)

```bash
agent-hand-bridge canvas '{"op":"add_node","id":"session:abc123","label":"My Project","kind":"Process"}'
agent-hand-bridge canvas '{"op":"add_node","id":"session:def456","label":"API Server","kind":"Process"}'
agent-hand-bridge canvas '{"op":"add_edge","from":"session:abc123","to":"session:def456","label":"calls"}'
```

### Query and inspect state

```bash
# List all nodes
agent-hand-bridge query nodes

# Filter nodes by kind
agent-hand-bridge query nodes --kind Decision

# Filter nodes by label substring
agent-hand-bridge query nodes --label "API"

# Get a specific node by ID
agent-hand-bridge query nodes --id "session:abc123"

# List all edges
agent-hand-bridge query edges

# Filter edges by label
agent-hand-bridge query edges --label "depends"

# Full state dump
agent-hand-bridge query state
```

### Undo mistakes

```bash
agent-hand-bridge canvas '{"op":"undo"}'
agent-hand-bridge canvas '{"op":"redo"}'
```

## Using with the full CLI

The `agent-hand` CLI also has canvas subcommands (higher-level, requires tokio):

```bash
agent-hand canvas add-node --id n1 --label "Task" --kind process
agent-hand canvas remove-node --id n1
agent-hand canvas add-edge --from n1 --to n2 --label "depends"
agent-hand canvas remove-edge --from n1 --to n2
agent-hand canvas layout --direction top-down
agent-hand canvas query nodes
agent-hand canvas batch --file ops.json
agent-hand canvas raw '{"op":"undo"}'
```

The CLI converts these to `CanvasOp` JSON and sends via the same socket.
For AI agents, prefer `agent-hand-bridge` for speed (~2ms startup vs ~50ms).

### clear_prefix

Atomically remove all nodes whose ID starts with a given prefix (and their edges).
Used by agent projections to clear stale rendering before re-drawing.

```json
{"op": "clear_prefix", "prefix": "ap_"}
```

| Field    | Type   | Required | Description                              |
|----------|--------|----------|------------------------------------------|
| `prefix` | string | yes      | ID prefix to match (e.g. `"ap_"`, `"wasm_"`) |

### query: viewport

Query the current viewport dimensions and position:

```json
{"op": "query", "what": "viewport"}
```

Returns:
```json
{"type": "viewport", "panel_cols": 80, "panel_rows": 24, "viewport_x": 0, "viewport_y": 0}
```

## Prefix Validation

When ops arrive from external sources (socket, WASM), the canvas engine enforces:

- Agent-generated nodes must use `ap_` or `wasm_` prefixes
- Batch size capped at 200 ops
- Projection node count capped at 100
- Ops targeting user-created nodes (no matching prefix) are rejected

## Error Handling

- If agent-hand is not running: `error: cannot connect to canvas socket`
- If JSON is malformed: `{"type":"error","message":"invalid JSON: ..."}`
- If node ID not found: `{"type":"error","message":"node not found: ..."}`
- If prefix violation: `{"type":"error","message":"prefix not allowed: ..."}`
- Check status first: `agent-hand-bridge ping`

## Related Skills

- **`canvas-render`** — How to read runtime artifacts and produce agent-driven canvas visualizations
- **`workspace-ops`** — Session and group management, bridge overview
