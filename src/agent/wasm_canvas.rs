//! WasmCanvasHost — persistent WASM canvas plugin host.
//!
//! Unlike WasmAnalyzer (stateless, per-invocation), this host keeps a WASM
//! instance alive and dispatches canvas events to it. The guest responds
//! with canvas operations that the host applies.
//!
//! ```text
//! Event Flow:
//!   canvas interaction ──► WasmCanvasHost.dispatch(event)
//!                                │
//!                                ▼
//!                          WASM guest on_event()
//!                                │
//!                                ▼
//!                          PluginOutput { canvas_ops, log }
//!                                │
//!                                ▼
//!                          host applies ops to canvas
//! ```

#![cfg(feature = "wasm")]

use std::path::Path;

use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

// ── Plugin Input/Output Types (host-side mirrors of SDK types) ──────

/// Input sent to the WASM plugin.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInput {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordination: Option<CoordinationData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas_summary: Option<CanvasSummary>,
    /// Host capabilities manifest (sent during init).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HostCapabilities>,
    /// Results from previously requested host operations (host_response event).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub host_results: Vec<HostRequestResult>,
}

/// Coordination data for the plugin.
#[derive(Debug, Clone, Default, serde::Serialize)]
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

/// Canvas state summary.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CanvasSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub node_ids: Vec<String>,
    /// Viewport dimensions (terminal cells available for canvas rendering).
    pub viewport_cols: u16,
    pub viewport_rows: u16,
    /// Current viewport scroll offset.
    pub viewport_x: i32,
    pub viewport_y: i32,
    /// Suggested LOD level based on node density vs viewport size.
    /// One of: "detail", "standard", "overview", "summary".
    pub suggested_lod: String,
}

/// Compute suggested LOD level from node count and viewport area.
pub fn compute_lod(node_count: usize, viewport_cols: u16, viewport_rows: u16) -> &'static str {
    // Estimate how many nodes fit comfortably in the viewport
    // Each node is ~18x3 cells, plus padding
    let viewport_area = (viewport_cols as usize) * (viewport_rows as usize);
    let node_area = 18 * 5; // node width * (height + padding)
    let capacity = viewport_area / node_area.max(1);

    match node_count {
        n if n <= capacity / 2 => "detail",
        n if n <= capacity => "standard",
        n if n <= capacity * 2 => "overview",
        _ => "summary",
    }
}

/// Output from the WASM plugin.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PluginOutput {
    pub canvas_ops: Vec<serde_json::Value>,
    /// Requests for the host to execute (CLI commands, API calls, etc.)
    #[serde(default)]
    pub host_requests: Vec<HostRequest>,
    #[serde(default)]
    pub log: Vec<String>,
}

// ── Host Capabilities ───────────────────────────────────────────────

/// Describes what the host can do — sent to the plugin during init.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HostCapabilities {
    pub version: String,
    pub features: Vec<String>,
    pub canvas_ops: Vec<String>,
    pub cli: Vec<CliCapability>,
    pub apis: Vec<ApiCapability>,
    pub skills: Vec<SkillCapability>,
    pub runtime: RuntimeInfo,
}

/// A CLI command the host can execute.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CliCapability {
    pub command: String,
    pub subcommands: Vec<String>,
    pub description: String,
}

/// An API endpoint the host exposes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiCapability {
    pub endpoint: String,
    pub params: Vec<String>,
    pub description: String,
}

/// A skill the plugin can invoke through the host.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillCapability {
    pub name: String,
    pub description: String,
}

/// Runtime environment information.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RuntimeInfo {
    pub runtime_dir: String,
    pub progress_dir: String,
    pub canvas_socket: String,
}

// ── Host Requests ───────────────────────────────────────────────────

/// A request from the plugin to the host for a deep operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostRequest {
    pub request_id: String,
    pub request_type: String,
    pub target: String,
    #[serde(default)]
    pub args: std::collections::HashMap<String, String>,
}

/// Result of a host request, sent back to the plugin.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HostRequestResult {
    pub request_id: String,
    pub success: bool,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

// ── Error Type ──────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum CanvasPluginError {
    #[error("WASM load error: {0}")]
    Load(String),
    #[error("WASM trap: {0}")]
    Trap(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("guest returned error status: {0}")]
    GuestError(i32),
}

// ── WasmCanvasHost ──────────────────────────────────────────────────

/// Persistent WASM canvas plugin instance.
///
/// Keeps the WASM module loaded and dispatches events to it.
/// Unlike WasmAnalyzer, this reuses the same Store/Instance across calls
/// to allow the guest to maintain state (if it chooses to).
pub struct WasmCanvasHost {
    store: Store<()>,
    #[allow(dead_code)]
    instance: Instance,
    memory: Memory,
    alloc_fn: TypedFunc<i32, i32>,
    on_event_fn: TypedFunc<(i32, i32), i32>,
    result_ptr_fn: TypedFunc<(), i32>,
    result_len_fn: TypedFunc<(), i32>,
    /// Cached capabilities manifest, built once on construction.
    capabilities: HostCapabilities,
}

impl WasmCanvasHost {
    /// Load a WASM canvas plugin from a file.
    pub fn from_file(path: &Path) -> Result<Self, CanvasPluginError> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, path)
            .map_err(|e| CanvasPluginError::Load(format!("file load: {}", e)))?;
        Self::from_module(engine, module)
    }

    /// Load a WASM canvas plugin from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CanvasPluginError> {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes)
            .map_err(|e| CanvasPluginError::Load(format!("compile: {}", e)))?;
        Self::from_module(engine, module)
    }

    fn from_module(engine: Engine, module: Module) -> Result<Self, CanvasPluginError> {
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| CanvasPluginError::Trap(format!("instantiation: {}", e)))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| CanvasPluginError::Trap("missing 'memory' export".into()))?;

        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| CanvasPluginError::Trap(format!("missing 'alloc': {}", e)))?;

        // Try on_event first, fall back to analyze
        let on_event_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "on_event")
            .or_else(|_| {
                instance.get_typed_func::<(i32, i32), i32>(&mut store, "analyze")
            })
            .map_err(|e| {
                CanvasPluginError::Trap(format!("missing 'on_event' or 'analyze': {}", e))
            })?;

        let result_ptr_fn = instance
            .get_typed_func::<(), i32>(&mut store, "result_ptr")
            .map_err(|e| CanvasPluginError::Trap(format!("missing 'result_ptr': {}", e)))?;

        let result_len_fn = instance
            .get_typed_func::<(), i32>(&mut store, "result_len")
            .map_err(|e| CanvasPluginError::Trap(format!("missing 'result_len': {}", e)))?;

        Ok(Self {
            store,
            instance,
            memory,
            alloc_fn,
            on_event_fn,
            result_ptr_fn,
            result_len_fn,
            capabilities: Self::build_default_capabilities(),
        })
    }

    /// Build the default host capabilities manifest.
    fn build_default_capabilities() -> HostCapabilities {
        HostCapabilities {
            version: env!("CARGO_PKG_VERSION").to_string(),
            features: vec!["wasm".to_string()],
            canvas_ops: vec![
                "add_node".to_string(),
                "remove_node".to_string(),
                "update_node".to_string(),
                "add_edge".to_string(),
                "remove_edge".to_string(),
                "layout".to_string(),
                "batch".to_string(),
            ],
            cli: vec![CliCapability {
                command: "agent-hand-bridge".to_string(),
                subcommands: vec![
                    "canvas".to_string(),
                    "query".to_string(),
                    "session".to_string(),
                ],
                description: "Bridge CLI for canvas and session operations".to_string(),
            }],
            apis: vec![
                ApiCapability {
                    endpoint: "read_progress".to_string(),
                    params: vec!["session_key".to_string()],
                    description: "Read progress entries for a session".to_string(),
                },
                ApiCapability {
                    endpoint: "read_cold_memory".to_string(),
                    params: vec![],
                    description: "Read cold memory records".to_string(),
                },
                ApiCapability {
                    endpoint: "query_canvas".to_string(),
                    params: vec!["query".to_string()],
                    description: "Query current canvas state".to_string(),
                },
            ],
            skills: vec![
                SkillCapability {
                    name: "canvas-ops".to_string(),
                    description: "JSON API for canvas manipulation".to_string(),
                },
                SkillCapability {
                    name: "workspace-ops".to_string(),
                    description: "Unified session and canvas management".to_string(),
                },
            ],
            runtime: RuntimeInfo::default(),
        }
    }

    /// Set runtime paths in the capabilities manifest.
    pub fn set_runtime_info(&mut self, runtime_dir: &str, progress_dir: &str, canvas_socket: &str) {
        self.capabilities.runtime = RuntimeInfo {
            runtime_dir: runtime_dir.to_string(),
            progress_dir: progress_dir.to_string(),
            canvas_socket: canvas_socket.to_string(),
        };
    }

    /// Dispatch an event to the plugin and collect canvas ops.
    pub fn dispatch(&mut self, input: &PluginInput) -> Result<PluginOutput, CanvasPluginError> {
        // Serialize input
        let input_json = serde_json::to_vec(input)
            .map_err(|e| CanvasPluginError::Serialization(format!("input: {}", e)))?;

        // Allocate in guest
        let input_len = input_json.len() as i32;
        let ptr = self
            .alloc_fn
            .call(&mut self.store, input_len)
            .map_err(|e| CanvasPluginError::Trap(format!("alloc: {}", e)))?;

        // Copy input into guest memory
        self.memory.data_mut(&mut self.store)
            [ptr as usize..ptr as usize + input_json.len()]
            .copy_from_slice(&input_json);

        // Call on_event
        let status = self
            .on_event_fn
            .call(&mut self.store, (ptr, input_len))
            .map_err(|e| CanvasPluginError::Trap(format!("on_event: {}", e)))?;

        if status != 0 {
            return Err(CanvasPluginError::GuestError(status));
        }

        // Read result
        let result_ptr = self
            .result_ptr_fn
            .call(&mut self.store, ())
            .map_err(|e| CanvasPluginError::Trap(format!("result_ptr: {}", e)))?
            as usize;
        let result_len = self
            .result_len_fn
            .call(&mut self.store, ())
            .map_err(|e| CanvasPluginError::Trap(format!("result_len: {}", e)))?
            as usize;

        let result_bytes = &self.memory.data(&self.store)[result_ptr..result_ptr + result_len];
        let output: PluginOutput = serde_json::from_slice(result_bytes)
            .map_err(|e| CanvasPluginError::Serialization(format!("output: {}", e)))?;

        Ok(output)
    }

    /// Send init event (includes capabilities manifest).
    pub fn init(&mut self) -> Result<PluginOutput, CanvasPluginError> {
        self.dispatch(&PluginInput {
            event: "init".to_string(),
            node_id: None,
            edge_from: None,
            edge_to: None,
            coordination: None,
            canvas_summary: None,
            capabilities: Some(self.capabilities.clone()),
            host_results: vec![],
        })
    }

    /// Send coordination update event.
    pub fn on_coordination_update(
        &mut self,
        coord: CoordinationData,
        canvas_summary: Option<CanvasSummary>,
    ) -> Result<PluginOutput, CanvasPluginError> {
        self.dispatch(&PluginInput {
            event: "coordination_update".to_string(),
            node_id: None,
            edge_from: None,
            edge_to: None,
            coordination: Some(coord),
            canvas_summary,
            capabilities: None,
            host_results: vec![],
        })
    }

    /// Send node click event.
    pub fn on_node_click(
        &mut self,
        node_id: &str,
        canvas_summary: Option<CanvasSummary>,
    ) -> Result<PluginOutput, CanvasPluginError> {
        self.dispatch(&PluginInput {
            event: "node_click".to_string(),
            node_id: Some(node_id.to_string()),
            edge_from: None,
            edge_to: None,
            coordination: None,
            canvas_summary,
            capabilities: None,
            host_results: vec![],
        })
    }

    /// Send host response event with results from previous host requests.
    pub fn send_host_results(
        &mut self,
        results: Vec<HostRequestResult>,
    ) -> Result<PluginOutput, CanvasPluginError> {
        self.dispatch(&PluginInput {
            event: "host_response".to_string(),
            node_id: None,
            edge_from: None,
            edge_to: None,
            coordination: None,
            canvas_summary: None,
            capabilities: None,
            host_results: results,
        })
    }
}
