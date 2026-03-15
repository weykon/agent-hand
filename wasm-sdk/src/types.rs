//! Shared types for the WASM plugin ABI.
//!
//! These types define the contract between host (agent-hand) and guest (.wasm module).
//! They are serialized as JSON across the WASM boundary.

use serde::{Deserialize, Serialize};

// ── Plugin Input ────────────────────────────────────────────────────

/// Input sent from host to guest on each event dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInput {
    /// Event type: "init", "coordination_update", "node_click", "edge_click",
    /// "refresh", "host_response", "shutdown".
    pub event: String,

    /// Node ID (for node_click events).
    #[serde(default)]
    pub node_id: Option<String>,

    /// Edge endpoints (for edge_click events).
    #[serde(default)]
    pub edge_from: Option<String>,
    #[serde(default)]
    pub edge_to: Option<String>,

    /// Current coordination data (for coordination_update and init).
    #[serde(default)]
    pub coordination: Option<CoordinationData>,

    /// Summary of current canvas state.
    #[serde(default)]
    pub canvas_summary: Option<CanvasSummary>,

    /// Host capabilities manifest (sent during init).
    #[serde(default)]
    pub capabilities: Option<HostCapabilities>,

    /// Results from previously requested host operations (host_response event).
    #[serde(default)]
    pub host_results: Vec<HostRequestResult>,
}

/// Coordination data from the Hot Brain pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoordinationData {
    pub blockers: Vec<String>,
    pub affected_targets: Vec<String>,
    pub decisions: Vec<String>,
    pub findings: Vec<String>,
    pub next_steps: Vec<String>,
    pub urgency: String,
    pub session_id: String,
    pub trace_id: String,
}

/// Summary of current canvas state (lightweight, not full state).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanvasSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub node_ids: Vec<String>,
    /// Viewport dimensions (terminal cells available for canvas rendering).
    #[serde(default)]
    pub viewport_cols: u16,
    #[serde(default)]
    pub viewport_rows: u16,
    /// Current viewport scroll offset.
    #[serde(default)]
    pub viewport_x: i32,
    #[serde(default)]
    pub viewport_y: i32,
    /// Suggested LOD level based on node density vs viewport size.
    #[serde(default)]
    pub suggested_lod: String,
}

// ── Plugin Output ───────────────────────────────────────────────────

/// Output from guest to host after processing an event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginOutput {
    /// Canvas operations to apply.
    pub canvas_ops: Vec<CanvasOp>,

    /// Requests for the host to execute (CLI commands, API calls, etc.)
    #[serde(default)]
    pub host_requests: Vec<HostRequest>,

    /// Log messages (for debugging/tracing).
    #[serde(default)]
    pub log: Vec<String>,
}

// ── Host Capabilities ───────────────────────────────────────────────

/// Describes what the host can do — sent to the plugin during init.
///
/// The plugin uses this to discover available operations and adapt its
/// behavior to the host's feature set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCapabilities {
    /// agent-hand version.
    pub version: String,

    /// Active feature flags (e.g. "pro", "max", "wasm").
    pub features: Vec<String>,

    /// Available canvas operation types.
    pub canvas_ops: Vec<String>,

    /// CLI commands the host can execute on behalf of the plugin.
    pub cli: Vec<CliCapability>,

    /// API endpoints the host exposes to plugins.
    pub apis: Vec<ApiCapability>,

    /// Skills the plugin can invoke.
    pub skills: Vec<SkillCapability>,

    /// Runtime paths (for reference, not direct access).
    pub runtime: RuntimeInfo,
}

/// A CLI command the host can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCapability {
    /// Base command (e.g. "agent-hand-bridge").
    pub command: String,
    /// Available subcommands (e.g. ["canvas", "query", "session"]).
    pub subcommands: Vec<String>,
    /// Human-readable description.
    pub description: String,
}

/// An API endpoint the host exposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCapability {
    /// Endpoint name (e.g. "read_progress", "read_cold_memory", "query_canvas").
    pub endpoint: String,
    /// Required parameters.
    pub params: Vec<String>,
    /// Human-readable description.
    pub description: String,
}

/// A skill the plugin can invoke through the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCapability {
    /// Skill name (e.g. "canvas-ops", "workspace-ops").
    pub name: String,
    /// Human-readable description.
    pub description: String,
}

/// Runtime environment information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// Runtime directory path (audit files, snapshots).
    pub runtime_dir: String,
    /// Progress directory path.
    pub progress_dir: String,
    /// Canvas socket path (if available).
    pub canvas_socket: String,
}

// ── Host Requests ───────────────────────────────────────────────────

/// A request from the plugin to the host for a deep operation.
///
/// The host processes these after receiving the PluginOutput,
/// and sends results back in the next event as `host_results`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRequest {
    /// Unique ID for correlating request ↔ response.
    pub request_id: String,

    /// Request type: "cli", "api", "read_file", "skill".
    pub request_type: String,

    /// For CLI: the command string (e.g. "agent-hand-bridge query nodes").
    /// For API: the endpoint name (e.g. "read_progress").
    /// For skill: the skill name (e.g. "canvas-ops").
    pub target: String,

    /// Arguments as key-value pairs.
    #[serde(default)]
    pub args: std::collections::HashMap<String, String>,
}

/// Result of a host request, sent back to the plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostRequestResult {
    /// Correlates to HostRequest.request_id.
    pub request_id: String,
    /// Whether the request succeeded.
    pub success: bool,
    /// Result data (JSON value for flexibility).
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    /// Error message if failed.
    #[serde(default)]
    pub error: Option<String>,
}

// ── Canvas Operations ───────────────────────────────────────────────

/// Canvas operation — mirrors the host's CanvasOp for JSON compatibility.
///
/// The host deserializes these and forwards to the canvas engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CanvasOp {
    AddNode {
        id: String,
        label: String,
        #[serde(default = "default_kind")]
        kind: String,
        #[serde(default)]
        pos: Option<(u16, u16)>,
        #[serde(default)]
        content: Option<String>,
    },
    RemoveNode {
        id: String,
    },
    UpdateNode {
        id: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        pos: Option<(u16, u16)>,
        #[serde(default)]
        content: Option<String>,
    },
    AddEdge {
        from: String,
        to: String,
        #[serde(default)]
        label: Option<String>,
    },
    RemoveEdge {
        from: String,
        to: String,
    },
    Layout {
        direction: String,
    },
    Batch {
        ops: Vec<CanvasOp>,
    },
}

fn default_kind() -> String {
    "Process".to_string()
}

// ── Builder Helpers ─────────────────────────────────────────────────

impl CanvasOp {
    /// Add a simple node.
    pub fn add_node(id: &str, label: &str, kind: &str) -> Self {
        Self::AddNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            pos: None,
            content: None,
        }
    }

    /// Add a node with content (like a Note).
    pub fn add_note(id: &str, label: &str, content: &str) -> Self {
        Self::AddNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: "Note".to_string(),
            pos: None,
            content: Some(content.to_string()),
        }
    }

    /// Add a node at a specific position.
    pub fn add_node_at(id: &str, label: &str, kind: &str, x: u16, y: u16) -> Self {
        Self::AddNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            pos: Some((x, y)),
            content: None,
        }
    }

    /// Add an edge between two nodes.
    pub fn add_edge(from: &str, to: &str, label: Option<&str>) -> Self {
        Self::AddEdge {
            from: from.to_string(),
            to: to.to_string(),
            label: label.map(String::from),
        }
    }

    /// Update a node's content.
    pub fn update_content(id: &str, content: &str) -> Self {
        Self::UpdateNode {
            id: id.to_string(),
            label: None,
            kind: None,
            pos: None,
            content: Some(content.to_string()),
        }
    }

    /// Update a node's label.
    pub fn update_label(id: &str, label: &str) -> Self {
        Self::UpdateNode {
            id: id.to_string(),
            label: Some(label.to_string()),
            kind: None,
            pos: None,
            content: None,
        }
    }

    /// Remove a node.
    pub fn remove(id: &str) -> Self {
        Self::RemoveNode {
            id: id.to_string(),
        }
    }

    /// Auto-layout top-down.
    pub fn layout_top_down() -> Self {
        Self::Layout {
            direction: "top_down".to_string(),
        }
    }

    /// Auto-layout tiled grid (fills rows first, avoids overlap).
    pub fn layout_tiled() -> Self {
        Self::Layout {
            direction: "tiled".to_string(),
        }
    }

    /// Batch multiple operations.
    pub fn batch(ops: Vec<CanvasOp>) -> Self {
        Self::Batch { ops }
    }
}
