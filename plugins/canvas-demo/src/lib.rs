#![allow(static_mut_refs)]
//! Reference WASM canvas plugin for agent-hand.
//!
//! Demonstrates the full agent-driven canvas lifecycle:
//! - `init` → registers plugin, acknowledges capabilities
//! - `coordination_update` → builds a scheduler-style flowchart from coordination data
//! - `node_click` → expands/collapses node detail
//! - `host_response` → processes host request results
//!
//! Build: `cargo build --target wasm32-wasi --release`
//! Deploy: copy `target/wasm32-wasi/release/canvas_demo.wasm` to
//!         `~/.agent-hand/profiles/default/agent-runtime/plugins/canvas_plugin.wasm`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── SDK Types (mirror host types) ──────────────────────────────────

#[derive(Deserialize)]
struct PluginInput {
    event: String,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    coordination: Option<CoordinationData>,
    #[serde(default)]
    canvas_summary: Option<CanvasSummary>,
    #[serde(default)]
    capabilities: Option<serde_json::Value>,
    #[serde(default)]
    host_results: Vec<HostRequestResult>,
}

#[derive(Deserialize, Default)]
struct CoordinationData {
    #[serde(default)]
    blockers: Vec<String>,
    #[serde(default)]
    affected_targets: Vec<String>,
    #[serde(default)]
    decisions: Vec<String>,
    #[serde(default)]
    findings: Vec<String>,
    #[serde(default)]
    next_steps: Vec<String>,
    #[serde(default)]
    urgency: String,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    trace_id: String,
}

#[derive(Deserialize, Default)]
struct CanvasSummary {
    node_count: usize,
    #[allow(dead_code)]
    edge_count: usize,
    #[allow(dead_code)]
    node_ids: Vec<String>,
}

#[derive(Deserialize)]
struct HostRequestResult {
    request_id: String,
    success: bool,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Serialize)]
struct PluginOutput {
    canvas_ops: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    host_requests: Vec<HostRequest>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    log: Vec<String>,
}

#[derive(Serialize)]
struct HostRequest {
    request_id: String,
    request_type: String,
    target: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    args: HashMap<String, String>,
}

// ── Plugin State ───────────────────────────────────────────────────
static mut STATE: Option<PluginState> = None;

struct PluginState {
    expanded_nodes: std::collections::HashSet<String>,
    last_coordination: Option<CoordinationData>,
}

fn state() -> &'static mut PluginState {
    unsafe {
        STATE.get_or_insert_with(|| PluginState {
            expanded_nodes: std::collections::HashSet::new(),
            last_coordination: None,
        })
    }
}

// ── WASM Exports (host calls these) ────────────────────────────────

static mut RESULT_BUF: Vec<u8> = Vec::new();

/// Allocate `size` bytes in guest memory. Host writes input here.
#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let buf = vec![0u8; size as usize];
    let ptr = buf.as_ptr() as *mut u8;
    std::mem::forget(buf);
    ptr
}

/// Main event handler. Host writes JSON input at `ptr..ptr+len`,
/// guest processes it and stores result in RESULT_BUF.
/// Returns 0 on success, non-zero on error.
#[no_mangle]
pub extern "C" fn on_event(ptr: i32, len: i32) -> i32 {
    let input_bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };

    let input: PluginInput = match serde_json::from_slice(input_bytes) {
        Ok(v) => v,
        Err(e) => {
            let err_output = PluginOutput {
                canvas_ops: vec![],
                host_requests: vec![],
                log: vec![format!("parse error: {}", e)],
            };
            let bytes = serde_json::to_vec(&err_output).unwrap_or_default();
            unsafe { RESULT_BUF = bytes; }
            return 1;
        }
    };

    let output = handle_event(input);
    let bytes = serde_json::to_vec(&output).unwrap_or_default();
    unsafe { RESULT_BUF = bytes; }
    0
}

/// Return pointer to result buffer.
#[no_mangle]
pub extern "C" fn result_ptr() -> *const u8 {
    unsafe { RESULT_BUF.as_ptr() }
}

/// Return length of result buffer.
#[no_mangle]
pub extern "C" fn result_len() -> i32 {
    unsafe { RESULT_BUF.len() as i32 }
}

// ── Event Dispatch ─────────────────────────────────────────────────

fn handle_event(input: PluginInput) -> PluginOutput {
    match input.event.as_str() {
        "init" => handle_init(input),
        "coordination_update" => handle_coordination(input),
        "node_click" => handle_node_click(input),
        "host_response" => handle_host_response(input),
        other => PluginOutput {
            canvas_ops: vec![],
            host_requests: vec![],
            log: vec![format!("unknown event: {}", other)],
        },
    }
}

fn handle_init(input: PluginInput) -> PluginOutput {
    let cap_info = input.capabilities
        .map(|c| format!("{}", c))
        .unwrap_or_else(|| "none".into());
    PluginOutput {
        canvas_ops: vec![],
        host_requests: vec![],
        log: vec![
            "canvas-demo plugin initialized".into(),
            format!("capabilities: {}", &cap_info[..cap_info.len().min(200)]),
        ],
    }
}

fn handle_coordination(input: PluginInput) -> PluginOutput {
    let coord = match input.coordination {
        Some(c) => c,
        None => return PluginOutput {
            canvas_ops: vec![],
            host_requests: vec![],
            log: vec!["coordination_update without data".into()],
        },
    };

    // Determine LOD based on canvas node count
    let lod = match input.canvas_summary.as_ref().map(|s| s.node_count).unwrap_or(0) {
        0..=20 => "detail",
        21..=50 => "standard",
        _ => "overview",
    };

    let mut ops = Vec::new();

    // Clear previous projection nodes
    ops.push(serde_json::json!({"ClearPrefix": {"prefix": "wasm_"}}));

    // Header node
    let urgency_kind = match coord.urgency.as_str() {
        "Critical" | "critical" => "End",
        "High" | "high" => "Decision",
        _ => "Process",
    };
    ops.push(serde_json::json!({
        "AddNode": {
            "id": format!("wasm_hdr_{}", &coord.trace_id[..coord.trace_id.len().min(8)]),
            "label": format!("[{}] {}", coord.urgency, &coord.session_id[..coord.session_id.len().min(12)]),
            "kind": urgency_kind,
            "pos": [2, 1],
            "content": format!("Trace: {}\nUrgency: {}\nLOD: {}", coord.trace_id, coord.urgency, lod),
        }
    }));

    let mut row = 5u16;

    // Blockers column
    if !coord.blockers.is_empty() {
        ops.push(serde_json::json!({
            "AddNode": {
                "id": "wasm_blockers_hdr",
                "label": format!("Blockers ({})", coord.blockers.len()),
                "kind": "Note",
                "pos": [0, row],
            }
        }));
        row += 3;
        for (i, b) in coord.blockers.iter().enumerate() {
            let label = if lod == "overview" {
                format!("B{}", i + 1)
            } else {
                truncate(b, 24)
            };
            let id = format!("wasm_blocker_{}", i);
            ops.push(serde_json::json!({
                "AddNode": {
                    "id": id,
                    "label": label,
                    "kind": "End",
                    "pos": [0, row],
                    "content": if lod == "detail" { Some(b.clone()) } else { None },
                }
            }));
            row += if lod == "detail" { 5 } else { 3 };
        }
    }

    // Decisions column
    if !coord.decisions.is_empty() {
        row = 5;
        ops.push(serde_json::json!({
            "AddNode": {
                "id": "wasm_decisions_hdr",
                "label": format!("Decisions ({})", coord.decisions.len()),
                "kind": "Note",
                "pos": [24, row],
            }
        }));
        row += 3;
        for (i, d) in coord.decisions.iter().enumerate() {
            let id = format!("wasm_decision_{}", i);
            ops.push(serde_json::json!({
                "AddNode": {
                    "id": id,
                    "label": truncate(d, 24),
                    "kind": "Decision",
                    "pos": [24, row],
                    "content": if lod == "detail" { Some(d.clone()) } else { None },
                }
            }));
            row += if lod == "detail" { 5 } else { 3 };
        }
    }

    // Next steps column
    if !coord.next_steps.is_empty() {
        row = 5;
        ops.push(serde_json::json!({
            "AddNode": {
                "id": "wasm_nextsteps_hdr",
                "label": format!("Next Steps ({})", coord.next_steps.len()),
                "kind": "Note",
                "pos": [48, row],
            }
        }));
        row += 3;
        let mut prev_id: Option<String> = None;
        for (i, ns) in coord.next_steps.iter().enumerate() {
            let id = format!("wasm_next_{}", i);
            ops.push(serde_json::json!({
                "AddNode": {
                    "id": &id,
                    "label": truncate(ns, 24),
                    "kind": "Start",
                    "pos": [48, row],
                    "content": if lod == "detail" { Some(ns.clone()) } else { None },
                }
            }));
            // Chain next steps
            if let Some(ref prev) = prev_id {
                ops.push(serde_json::json!({
                    "AddEdge": { "from": prev, "to": &id }
                }));
            }
            prev_id = Some(id);
            row += if lod == "detail" { 5 } else { 3 };
        }
    }

    // Save coordination for node_click expansion
    state().last_coordination = Some(coord);

    PluginOutput {
        canvas_ops: ops,
        host_requests: vec![],
        log: vec![format!("rendered coordination at LOD={}", lod)],
    }
}

fn handle_node_click(input: PluginInput) -> PluginOutput {
    let node_id = match input.node_id {
        Some(id) => id,
        None => return PluginOutput {
            canvas_ops: vec![],
            host_requests: vec![],
            log: vec!["node_click without node_id".into()],
        },
    };

    let s = state();

    // Toggle expansion
    if s.expanded_nodes.contains(&node_id) {
        s.expanded_nodes.remove(&node_id);
        // Collapse: remove detail node
        let detail_id = format!("{}_detail", node_id);
        PluginOutput {
            canvas_ops: vec![serde_json::json!({"RemoveNode": {"id": detail_id}})],
            host_requests: vec![],
            log: vec![format!("collapsed {}", node_id)],
        }
    } else {
        s.expanded_nodes.insert(node_id.clone());
        // Expand: add detail node next to clicked node
        let detail_id = format!("{}_detail", node_id);

        // Find the relevant data from last coordination
        let content = find_detail_for_node(&node_id, s.last_coordination.as_ref());

        PluginOutput {
            canvas_ops: vec![
                serde_json::json!({
                    "AddNode": {
                        "id": &detail_id,
                        "label": "Detail",
                        "kind": "Note",
                        "content": content,
                    }
                }),
                serde_json::json!({
                    "AddEdge": { "from": &node_id, "to": &detail_id, "label": "detail" }
                }),
            ],
            host_requests: vec![],
            log: vec![format!("expanded {}", node_id)],
        }
    }
}

fn handle_host_response(input: PluginInput) -> PluginOutput {
    let mut log = Vec::new();
    for result in &input.host_results {
        if result.success {
            log.push(format!("host request {} succeeded", result.request_id));
        } else {
            log.push(format!(
                "host request {} failed: {}",
                result.request_id,
                result.error.as_deref().unwrap_or("unknown"),
            ));
        }
    }
    PluginOutput {
        canvas_ops: vec![],
        host_requests: vec![],
        log,
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

fn find_detail_for_node(node_id: &str, coord: Option<&CoordinationData>) -> String {
    let coord = match coord {
        Some(c) => c,
        None => return "No coordination data available".into(),
    };

    if node_id.starts_with("wasm_blocker_") {
        if let Some(idx) = node_id.strip_prefix("wasm_blocker_").and_then(|s| s.parse::<usize>().ok()) {
            if let Some(b) = coord.blockers.get(idx) {
                return format!("Blocker: {}\n\nSession: {}\nUrgency: {}", b, coord.session_id, coord.urgency);
            }
        }
    } else if node_id.starts_with("wasm_decision_") {
        if let Some(idx) = node_id.strip_prefix("wasm_decision_").and_then(|s| s.parse::<usize>().ok()) {
            if let Some(d) = coord.decisions.get(idx) {
                return format!("Decision: {}\n\nTargets: {}", d, coord.affected_targets.join(", "));
            }
        }
    } else if node_id.starts_with("wasm_next_") {
        if let Some(idx) = node_id.strip_prefix("wasm_next_").and_then(|s| s.parse::<usize>().ok()) {
            if let Some(ns) = coord.next_steps.get(idx) {
                return format!("Next Step: {}\n\nFindings: {}", ns, coord.findings.join(", "));
            }
        }
    }

    format!("Node: {}", node_id)
}
