//! Free-version stub types for the canvas module.
//!
//! Provides all type definitions needed for compilation without the full
//! canvas implementation. All methods are no-ops or return empty defaults.

use std::collections::{HashMap, HashSet};
use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use serde::{Deserialize, Serialize};

pub use super::socket::CanvasRequest;

/// Visual kind of a canvas node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Start,
    End,
    Process,
    Decision,
    Note,
}

impl NodeKind {
    pub fn indicator(self) -> &'static str {
        match self {
            Self::Start => "\u{25b6} ",
            Self::End => "\u{25a0} ",
            Self::Decision => "\u{25c7} ",
            Self::Process => "",
            Self::Note => "# ",
        }
    }

    pub fn border_type(self) -> ratatui::widgets::BorderType {
        match self {
            Self::Start | Self::End => ratatui::widgets::BorderType::Rounded,
            Self::Decision => ratatui::widgets::BorderType::Double,
            Self::Process | Self::Note => ratatui::widgets::BorderType::Plain,
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        match self {
            Self::Start => ratatui::style::Color::Green,
            Self::End => ratatui::style::Color::Red,
            Self::Process => ratatui::style::Color::Cyan,
            Self::Decision => ratatui::style::Color::Yellow,
            Self::Note => ratatui::style::Color::DarkGray,
        }
    }
}

/// Data stored in each graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    pub id: String,
    pub label: String,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_source_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_source_type: Option<String>,
    #[serde(skip)]
    pub status_color: Option<ratatui::style::Color>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Data stored in each graph edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeData {
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship_id: Option<String>,
}

/// A named group of nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasGroup {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<String>,
}

/// Viewport offset for panning the canvas
#[derive(Debug, Clone, Copy, Default)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
}

/// Drag state for grab/drop interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    None,
    Dragging(NodeIndex),
}

/// Canvas mode: user-editable canvas or agent-generated visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasView {
    #[default]
    User,
    Agent,
}

impl CanvasView {
    pub fn next(self) -> Self { match self { Self::User => Self::Agent, Self::Agent => Self::User } }
    pub fn prev(self) -> Self { self.next() }
    pub fn label(self) -> &'static str { match self { Self::User => "User", Self::Agent => "Agent" } }
    pub fn index(self) -> usize { match self { Self::User => 0, Self::Agent => 1 } }
    pub fn from_index(i: usize) -> Option<Self> { match i { 0 => Some(Self::User), 1 => Some(Self::Agent), _ => None } }
}

/// Canvas interaction mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasMode {
    #[default]
    Auto,
    Free,
}

/// Layout direction for auto-layout
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LayoutDirection {
    TopDown,
    LeftRight,
}

fn default_process_kind() -> NodeKind { NodeKind::Process }
fn default_layout_direction() -> LayoutDirection { LayoutDirection::TopDown }

/// Operations that can be applied to the canvas
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CanvasOp {
    AddNode { id: String, label: String, #[serde(default = "default_process_kind")] kind: NodeKind, pos: Option<(u16, u16)>, #[serde(default, skip_serializing_if = "Option::is_none")] content: Option<String> },
    RemoveNode { id: String },
    UpdateNode { id: String, #[serde(skip_serializing_if = "Option::is_none")] label: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] kind: Option<NodeKind>, #[serde(skip_serializing_if = "Option::is_none")] pos: Option<(u16, u16)>, #[serde(default, skip_serializing_if = "Option::is_none")] content: Option<String> },
    AddEdge { from: String, to: String, #[serde(skip_serializing_if = "Option::is_none")] label: Option<String>, #[serde(default, skip_serializing_if = "Option::is_none")] relationship_id: Option<String> },
    UpdateEdge { from: String, to: String, #[serde(skip_serializing_if = "Option::is_none")] label: Option<String> },
    RemoveEdge { from: String, to: String },
    Layout { #[serde(default = "default_layout_direction")] direction: LayoutDirection },
    Batch { ops: Vec<CanvasOp> },
    Query { what: String, #[serde(default)] kind: Option<String>, #[serde(default)] label_contains: Option<String>, #[serde(default)] id: Option<String> },
    ClearPrefix { prefix: String },
    SetViewport { x: i32, y: i32 },
    SetMetadata { node_id: String, key: String, value: Option<String> },
    AddGroup { id: String, label: String, node_ids: Vec<String> },
    RemoveGroup { id: String },
    MoveGroup { id: String, dx: i32, dy: i32 },
    RegisterNamespace { name: String, prefix: String },
    Undo,
    Redo,
}

/// Response sent back over the socket after processing a CanvasOp
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasResponse {
    Ok { message: String },
    NodeList { nodes: Vec<NodeInfo> },
    EdgeList { edges: Vec<EdgeInfo> },
    State { json: serde_json::Value },
    Viewport { panel_cols: u16, panel_rows: u16, viewport_x: i32, viewport_y: i32 },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub label: String,
    pub kind: NodeKind,
    pub pos: (u16, u16),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_source_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship_id: Option<String>,
}

pub const MAX_EXTERNAL_BATCH_SIZE: usize = 200;
pub const MAX_PROJECTION_NODES: usize = 100;
pub const EXTERNAL_PREFIXES: &[&str] = &["ap_", "wasm_"];
pub const NODE_WIDTH: u16 = 18;
pub const NODE_HEIGHT: u16 = 3;

/// Validate a canvas op from an external source.
pub fn validate_external_op(op: &CanvasOp, current_projection_count: usize) -> Result<(), String> {
    let mut new_node_count = 0;
    validate_op_recursive(op, &mut new_node_count)?;
    if current_projection_count + new_node_count > MAX_PROJECTION_NODES {
        return Err(format!(
            "projection node count would exceed limit: {} existing + {} new > {}",
            current_projection_count, new_node_count, MAX_PROJECTION_NODES,
        ));
    }
    Ok(())
}

fn validate_op_recursive(op: &CanvasOp, new_node_count: &mut usize) -> Result<(), String> {
    match op {
        CanvasOp::AddNode { id, .. } => {
            if !EXTERNAL_PREFIXES.iter().any(|p| id.starts_with(p)) {
                return Err(format!("node ID '{}' must start with one of: {:?}", id, EXTERNAL_PREFIXES));
            }
            *new_node_count += 1;
        }
        CanvasOp::UpdateNode { id, .. } | CanvasOp::SetMetadata { node_id: id, .. } => {
            if !EXTERNAL_PREFIXES.iter().any(|p| id.starts_with(p)) {
                return Err(format!("node ID '{}' must start with one of: {:?}", id, EXTERNAL_PREFIXES));
            }
        }
        CanvasOp::AddGroup { id, .. } | CanvasOp::RemoveGroup { id } | CanvasOp::MoveGroup { id, .. } => {
            if !EXTERNAL_PREFIXES.iter().any(|p| id.starts_with(p)) {
                return Err(format!("group ID '{}' must start with one of: {:?}", id, EXTERNAL_PREFIXES));
            }
        }
        CanvasOp::Batch { ops } => {
            if ops.len() > MAX_EXTERNAL_BATCH_SIZE {
                return Err(format!("batch size {} exceeds maximum {}", ops.len(), MAX_EXTERNAL_BATCH_SIZE));
            }
            for sub_op in ops {
                validate_op_recursive(sub_op, new_node_count)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn node_dimensions(node: &NodeData) -> (u16, u16) {
    if let Some(ref content) = node.content {
        let lines: Vec<&str> = content.lines().collect();
        let max_line_width = lines.iter().map(|l| l.len()).max().unwrap_or(0) as u16;
        let w = (max_line_width + 4).max(NODE_WIDTH).min(120);
        let h = (lines.len() as u16 + 2).max(NODE_HEIGHT).min(60);
        (w, h)
    } else {
        (NODE_WIDTH, NODE_HEIGHT)
    }
}

pub fn canvas_filename_for_group(group_path: &str) -> String {
    let trimmed = group_path.trim();
    if trimmed.is_empty() || trimmed == "default" {
        "_default".to_string()
    } else {
        trimmed.replace('/', "__")
    }
}

/// A registered agent canvas namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNamespace {
    pub name: String,
    pub prefix: String,
}

/// Main canvas state (stub — no-op methods for free version)
pub struct CanvasState {
    pub graph: DiGraph<NodeData, EdgeData>,
    pub positions: HashMap<NodeIndex, (u16, u16)>,
    pub viewport: Viewport,
    pub selection: HashSet<NodeIndex>,
    pub cursor_pos: (u16, u16),
    pub drag: DragState,
    pub connect_source: Option<NodeIndex>,
    pub show_help: bool,
    pub editing: Option<(NodeIndex, crate::ui::TextInput)>,
    pub editing_edge: Option<(EdgeIndex, crate::ui::TextInput)>,
    pub adding_node: bool,
    pub mode: CanvasMode,
    id_index: HashMap<String, NodeIndex>,
    undo_stack: Vec<CanvasOp>,
    redo_stack: Vec<CanvasOp>,
    pub selected_edge: Option<EdgeIndex>,
    pub current_view: CanvasView,
    pub show_relationship_edges: bool,
    pub relationship_types: HashMap<String, crate::session::RelationType>,
    pub created_at: std::time::Instant,
    pub panel_cols: u16,
    pub panel_rows: u16,
    pub groups: HashMap<String, CanvasGroup>,
    pub agent_namespaces: Vec<AgentNamespace>,
    pub active_namespace: usize,
}

impl Default for CanvasState {
    fn default() -> Self { Self::new() }
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            positions: HashMap::new(),
            viewport: Viewport::default(),
            selection: HashSet::new(),
            cursor_pos: (0, 0),
            drag: DragState::None,
            connect_source: None,
            show_help: false,
            editing: None,
            editing_edge: None,
            adding_node: false,
            mode: CanvasMode::Auto,
            id_index: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            selected_edge: None,
            current_view: CanvasView::User,
            show_relationship_edges: false,
            relationship_types: HashMap::new(),
            created_at: std::time::Instant::now(),
            panel_cols: 0,
            panel_rows: 0,
            groups: HashMap::new(),
            agent_namespaces: Vec::new(),
            active_namespace: 0,
        }
    }

    /// No-op apply_op for free version
    pub fn apply_op(&mut self, _op: CanvasOp) -> CanvasResponse {
        CanvasResponse::Error { message: "Canvas is a Pro feature".to_string() }
    }

    pub fn is_editing(&self) -> bool { self.editing.is_some() || self.editing_edge.is_some() }
    pub fn is_projection_view(&self) -> bool { self.current_view == CanvasView::Agent }
    pub fn session_id_at_cursor(&self) -> Option<String> { None }
    pub fn selected_edge_relationship_id(&self) -> Option<&str> { None }
    pub fn projection_node_count(&self) -> usize { 0 }
    pub fn clear_projection_nodes(&mut self) {}
    pub fn handle_op(&mut self, op: CanvasOp) -> CanvasResponse {
        self.apply_op(op)
    }
}
