//! End-to-end integration test for the WASM canvas plugin pipeline.
//!
//! Loads the compiled demo_canvas_plugin.wasm and verifies:
//! 1. init → creates dashboard node
//! 2. coordination_update → creates blocker/target/decision nodes
//! 3. node_click → expands detail node
//! 4. node_click again → collapses detail node (toggle)
//!
//! Requires: `--features wasm` and the demo plugin compiled at
//! `tests/fixtures/demo_canvas_plugin.wasm`

#![cfg(feature = "wasm")]

use std::path::PathBuf;

use agent_hand::agent::wasm_canvas::*;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("demo_canvas_plugin.wasm")
}

#[test]
fn load_demo_plugin() {
    let path = fixture_path();
    assert!(path.exists(), "demo plugin .wasm should exist at {:?}", path);
    let host = WasmCanvasHost::from_file(&path);
    assert!(host.is_ok(), "should load demo plugin: {:?}", host.err());
}

#[test]
fn init_creates_dashboard_node() {
    let mut host = WasmCanvasHost::from_file(&fixture_path()).unwrap();

    let output = host.init().unwrap();

    // Should have at least one canvas op (AddNode for dashboard)
    assert!(
        !output.canvas_ops.is_empty(),
        "init should produce canvas ops"
    );

    // First op should be an add_node with id "wasm_dashboard"
    let first_op = &output.canvas_ops[0];
    assert_eq!(
        first_op.get("op").and_then(|v| v.as_str()),
        Some("add_node"),
        "first op should be add_node"
    );
    assert_eq!(
        first_op.get("id").and_then(|v| v.as_str()),
        Some("wasm_dashboard"),
        "should create wasm_dashboard node"
    );

    // Should have log message
    assert!(
        output.log.iter().any(|l| l.contains("dashboard")),
        "should log dashboard init"
    );
}

#[test]
fn coordination_update_creates_blocker_nodes() {
    let mut host = WasmCanvasHost::from_file(&fixture_path()).unwrap();

    // Init first
    let _ = host.init().unwrap();

    // Send coordination update with 2 blockers
    let coord = CoordinationData {
        blockers: vec!["db timeout".to_string(), "auth failure".to_string()],
        affected_targets: vec!["api-server".to_string()],
        decisions: vec!["switch to JWT".to_string()],
        findings: vec![],
        next_steps: vec![],
        urgency: "high".to_string(),
        session_id: "sid-test".to_string(),
        trace_id: "trace-test".to_string(),
    };

    let output = host.on_coordination_update(coord, None).unwrap();

    // Should create nodes for blockers, targets, decisions
    let add_ops: Vec<_> = output
        .canvas_ops
        .iter()
        .filter(|op| op.get("op").and_then(|v| v.as_str()) == Some("add_node"))
        .collect();

    // At least 2 blocker nodes + 1 target + 1 decision = 4
    assert!(
        add_ops.len() >= 4,
        "should create nodes for blockers/targets/decisions, got {}",
        add_ops.len()
    );

    // Check blocker node IDs
    let blocker_ids: Vec<_> = add_ops
        .iter()
        .filter_map(|op| op.get("id").and_then(|v| v.as_str()))
        .filter(|id| id.starts_with("wasm_v_blocker_"))
        .collect();
    assert_eq!(blocker_ids.len(), 2, "should have 2 blocker nodes");

    // Check edges connecting dashboard to blockers
    let edge_ops: Vec<_> = output
        .canvas_ops
        .iter()
        .filter(|op| op.get("op").and_then(|v| v.as_str()) == Some("add_edge"))
        .filter(|op| {
            op.get("from").and_then(|v| v.as_str()) == Some("wasm_dashboard")
        })
        .collect();
    assert!(
        edge_ops.len() >= 4,
        "should have edges from dashboard to each visualization node"
    );

    // Check log
    assert!(
        output.log.iter().any(|l| l.contains("2 blockers")),
        "log should mention blocker count"
    );
}

#[test]
fn node_click_expands_then_collapses() {
    let mut host = WasmCanvasHost::from_file(&fixture_path()).unwrap();

    // Init
    let _ = host.init().unwrap();

    // Click dashboard node — should expand
    let output = host
        .on_node_click(
            "wasm_dashboard",
            Some(CanvasSummary {
                node_count: 1,
                edge_count: 0,
                node_ids: vec!["wasm_dashboard".to_string()],
                viewport_cols: 80,
                viewport_rows: 24,
                viewport_x: 0,
                viewport_y: 0,
                suggested_lod: "detail".to_string(),
            }),
        )
        .unwrap();

    // Should create a detail node
    let add_ops: Vec<_> = output
        .canvas_ops
        .iter()
        .filter(|op| op.get("op").and_then(|v| v.as_str()) == Some("add_node"))
        .collect();
    assert!(
        !add_ops.is_empty(),
        "click should create detail node"
    );

    let detail_id = add_ops[0]
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(
        detail_id.contains("detail"),
        "detail node ID should contain 'detail'"
    );
    assert!(
        output.log.iter().any(|l| l.contains("expanded")),
        "log should say expanded"
    );

    // Click again with detail node in canvas — should collapse
    let output2 = host
        .on_node_click(
            "wasm_dashboard",
            Some(CanvasSummary {
                node_count: 2,
                edge_count: 1,
                node_ids: vec![
                    "wasm_dashboard".to_string(),
                    detail_id.to_string(),
                ],
                viewport_cols: 80,
                viewport_rows: 24,
                viewport_x: 0,
                viewport_y: 0,
                suggested_lod: "detail".to_string(),
            }),
        )
        .unwrap();

    // Should remove the detail node
    let remove_ops: Vec<_> = output2
        .canvas_ops
        .iter()
        .filter(|op| op.get("op").and_then(|v| v.as_str()) == Some("remove_node"))
        .collect();
    assert!(
        !remove_ops.is_empty(),
        "second click should remove detail node (collapse)"
    );
    assert!(
        output2.log.iter().any(|l| l.contains("collapsed")),
        "log should say collapsed"
    );
}

#[test]
fn non_wasm_node_click_ignored() {
    let mut host = WasmCanvasHost::from_file(&fixture_path()).unwrap();
    let _ = host.init().unwrap();

    // Click a non-wasm node — should produce no ops
    let output = host.on_node_click("session:abc123", None).unwrap();
    assert!(
        output.canvas_ops.is_empty(),
        "non-wasm node click should produce no ops"
    );
}

#[test]
fn full_pipeline_init_update_click() {
    let mut host = WasmCanvasHost::from_file(&fixture_path()).unwrap();

    // 1. Init
    let init_out = host.init().unwrap();
    assert!(!init_out.canvas_ops.is_empty());

    // 2. Coordination update
    let coord = CoordinationData {
        blockers: vec!["memory leak".to_string()],
        affected_targets: vec!["worker-1".to_string(), "worker-2".to_string()],
        decisions: vec![],
        findings: vec!["cache miss rate high".to_string()],
        next_steps: vec!["profile memory".to_string()],
        urgency: "critical".to_string(),
        session_id: "sid-pipeline".to_string(),
        trace_id: "trace-pipeline".to_string(),
    };
    let update_out = host.on_coordination_update(coord, None).unwrap();
    assert!(!update_out.canvas_ops.is_empty());

    // 3. Click blocker node
    let click_out = host
        .on_node_click(
            "wasm_v_blocker_0",
            Some(CanvasSummary {
                node_count: 5,
                edge_count: 4,
                node_ids: vec![
                    "wasm_dashboard".to_string(),
                    "wasm_v_blocker_0".to_string(),
                    "wasm_v_target_0".to_string(),
                    "wasm_v_target_1".to_string(),
                ],
                viewport_cols: 80,
                viewport_rows: 24,
                viewport_x: 0,
                viewport_y: 0,
                suggested_lod: "detail".to_string(),
            }),
        )
        .unwrap();

    // Should expand blocker with detail
    assert!(
        !click_out.canvas_ops.is_empty(),
        "blocker click should produce detail ops"
    );
    assert!(
        click_out.log.iter().any(|l| l.contains("expanded")),
        "should expand blocker detail"
    );
}
