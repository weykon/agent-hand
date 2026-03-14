//! Demo WASM Canvas Plugin — Blocker Dashboard
//!
//! This plugin demonstrates the WASM ↔ Canvas pipeline:
//!
//! - `init` → creates a dashboard root node
//! - `coordination_update` → visualizes blockers, targets, decisions as canvas nodes
//! - `node_click` → expands clicked node with detail sub-nodes
//!
//! ```text
//!  ┌──────────────────┐
//!  │   📊 Dashboard    │ ◄─ root node (always present)
//!  └────────┬─────────┘
//!           │
//!     ┌─────┼──────┐
//!     ▼     ▼      ▼
//!  ┌──────┐ ┌────┐ ┌──────┐
//!  │blocker│ │tgt │ │decide│ ◄─ generated from coordination data
//!  └──────┘ └────┘ └──────┘
//! ```

use agent_hand_wasm_sdk::*;

// Export the standard ABI (alloc, result_ptr, result_len)
export_abi!();

// ── Main Entry Point ────────────────────────────────────────────────

/// Handle an event from the host.
///
/// This is the main entry point called by the WASM host.
/// Returns 0 on success, 1 on error.
#[no_mangle]
pub extern "C" fn on_event(ptr: i32, len: i32) -> i32 {
    let input: PluginInput = match parse_input(ptr, len) {
        Ok(v) => v,
        Err(_) => return 1,
    };

    let output = handle_event(&input);
    set_result(&output);
    0
}

/// Also export as `analyze` for compatibility with the Analyzer ABI.
#[no_mangle]
pub extern "C" fn analyze(ptr: i32, len: i32) -> i32 {
    on_event(ptr, len)
}

// ── Event Handling Logic ────────────────────────────────────────────

fn handle_event(input: &PluginInput) -> PluginOutput {
    let mut output = PluginOutput::default();

    match input.event.as_str() {
        "init" => handle_init(&mut output, input),
        "coordination_update" => handle_coordination_update(&mut output, input),
        "node_click" => handle_node_click(&mut output, input),
        "refresh" => handle_refresh(&mut output, input),
        other => {
            output.log.push(format!("unknown event: {}", other));
        }
    }

    output
}

/// On init: create the dashboard root node.
fn handle_init(output: &mut PluginOutput, input: &PluginInput) {
    output.canvas_ops.push(CanvasOp::add_node_at(
        "wasm_dashboard",
        "Dashboard",
        "Start",
        2, 1,
    ));

    // Log capabilities if provided
    if let Some(caps) = &input.capabilities {
        output.log.push(format!(
            "dashboard initialized (host v{}, {} canvas ops, {} skills)",
            caps.version,
            caps.canvas_ops.len(),
            caps.skills.len(),
        ));
    } else {
        output.log.push("dashboard initialized".to_string());
    }
}

/// On coordination update: visualize blockers, targets, and decisions.
fn handle_coordination_update(output: &mut PluginOutput, input: &PluginInput) {
    let coord = match &input.coordination {
        Some(c) => c,
        None => {
            output.log.push("no coordination data".to_string());
            return;
        }
    };

    // Remove old visualization nodes (prefix: wasm_v_)
    if let Some(summary) = &input.canvas_summary {
        for id in &summary.node_ids {
            if id.starts_with("wasm_v_") {
                output.canvas_ops.push(CanvasOp::remove(id));
            }
        }
    }

    let mut col: u16 = 2;
    let row_blockers: u16 = 5;
    let row_targets: u16 = 9;
    let row_decisions: u16 = 13;

    // Update dashboard status
    let status_text = format!(
        "Session: {}\nBlockers: {}\nTargets: {}\nDecisions: {}\nUrgency: {}",
        coord.session_id,
        coord.blockers.len(),
        coord.affected_targets.len(),
        coord.decisions.len(),
        coord.urgency,
    );
    output.canvas_ops.push(CanvasOp::UpdateNode {
        id: "wasm_dashboard".to_string(),
        label: Some(format!("Dashboard [{}]", coord.urgency)),
        kind: None,
        pos: None,
        content: Some(status_text),
    });

    // Blocker nodes
    for (i, blocker) in coord.blockers.iter().enumerate() {
        let id = format!("wasm_v_blocker_{}", i);
        output.canvas_ops.push(CanvasOp::AddNode {
            id: id.clone(),
            label: truncate(blocker, 16),
            kind: "Decision".to_string(),
            pos: Some((col, row_blockers)),
            content: Some(blocker.clone()),
        });
        output.canvas_ops.push(CanvasOp::add_edge(
            "wasm_dashboard",
            &id,
            Some("blocker"),
        ));
        col += 22;
    }

    // Target nodes
    col = 2;
    for (i, target) in coord.affected_targets.iter().enumerate() {
        let id = format!("wasm_v_target_{}", i);
        output.canvas_ops.push(CanvasOp::AddNode {
            id: id.clone(),
            label: truncate(target, 16),
            kind: "Process".to_string(),
            pos: Some((col, row_targets)),
            content: Some(format!("Affected: {}", target)),
        });
        output.canvas_ops.push(CanvasOp::add_edge(
            "wasm_dashboard",
            &id,
            Some("affects"),
        ));
        col += 22;
    }

    // Decision nodes
    col = 2;
    for (i, decision) in coord.decisions.iter().enumerate() {
        let id = format!("wasm_v_decision_{}", i);
        output.canvas_ops.push(CanvasOp::add_note(
            &id,
            &truncate(decision, 16),
            decision,
        ));
        // Position manually since add_note doesn't take pos
        output.canvas_ops.push(CanvasOp::UpdateNode {
            id: id.clone(),
            label: None,
            kind: None,
            pos: Some((col, row_decisions)),
            content: None,
        });
        output.canvas_ops.push(CanvasOp::add_edge(
            "wasm_dashboard",
            &id,
            Some("decided"),
        ));
        col += 22;
    }

    // Layout
    output.canvas_ops.push(CanvasOp::layout_top_down());

    output.log.push(format!(
        "visualization updated: {} blockers, {} targets, {} decisions",
        coord.blockers.len(),
        coord.affected_targets.len(),
        coord.decisions.len(),
    ));
}

/// On node click: expand with detail sub-nodes.
fn handle_node_click(output: &mut PluginOutput, input: &PluginInput) {
    let node_id = match &input.node_id {
        Some(id) => id,
        None => return,
    };

    // Only expand wasm_ prefixed nodes
    if !node_id.starts_with("wasm_") {
        return;
    }

    let detail_id = format!("{}_detail", node_id);

    // Check if detail already exists (toggle behavior)
    if let Some(summary) = &input.canvas_summary {
        if summary.node_ids.contains(&detail_id) {
            // Remove detail node (collapse)
            output.canvas_ops.push(CanvasOp::remove(&detail_id));
            output.log.push(format!("collapsed: {}", node_id));
            return;
        }
    }

    // Expand: add detail node
    output.canvas_ops.push(CanvasOp::add_note(
        &detail_id,
        &format!("Detail: {}", node_id),
        &format!(
            "Expanded view for {}\n\
             Click again to collapse.\n\
             This node was generated by the\n\
             WASM canvas plugin.",
            node_id
        ),
    ));
    output.canvas_ops.push(CanvasOp::add_edge(
        node_id,
        &detail_id,
        Some("detail"),
    ));

    output.log.push(format!("expanded: {}", node_id));
}

/// On refresh: update dashboard with latest coordination data.
fn handle_refresh(output: &mut PluginOutput, input: &PluginInput) {
    // Refresh is the same as coordination_update
    handle_coordination_update(output, input);
}

// ── Helpers ─────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
