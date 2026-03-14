pub mod socket;

use serde::{Deserialize, Serialize};

use crate::session::{LabelColor, RelationType};

/// A control operation sent by external tools (bridge binary, scripts, AI agents).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlOp {
    // ── Session CRUD ──────────────────────────────────────────────
    AddSession {
        path: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        group: Option<String>,
        #[serde(default)]
        command: Option<String>,
    },
    RemoveSession {
        id: String,
    },
    ListSessions {
        #[serde(default)]
        group: Option<String>,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        status: Option<String>,
    },
    SessionInfo {
        id: String,
    },

    // ── Session lifecycle ─────────────────────────────────────────
    StartSession {
        id: String,
    },
    StopSession {
        id: String,
    },
    RestartSession {
        id: String,
    },
    ResumeSession {
        id: String,
    },
    InterruptSession {
        id: String,
    },
    SendPrompt {
        id: String,
        text: String,
    },

    // ── Session metadata ──────────────────────────────────────────
    RenameSession {
        id: String,
        title: String,
    },
    SetLabel {
        id: String,
        label: String,
        #[serde(default)]
        color: Option<LabelColor>,
    },
    MoveSession {
        id: String,
        group: String,
    },
    AddTag {
        id: String,
        tag: String,
    },
    RemoveTag {
        id: String,
        tag: String,
    },

    // ── Groups ────────────────────────────────────────────────────
    ListGroups,
    CreateGroup {
        path: String,
    },
    DeleteGroup {
        path: String,
    },
    RenameGroup {
        old_path: String,
        new_path: String,
    },

    // ── Relationships (Pro) ───────────────────────────────────────
    AddRelationship {
        session_a: String,
        session_b: String,
        #[serde(default)]
        relation_type: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    RemoveRelationship {
        id: String,
    },
    ListRelationships {
        #[serde(default)]
        session: Option<String>,
    },

    // ── Session inspection ────────────────────────────────────────
    /// Read the last N lines of a session's tmux pane output.
    ReadPane {
        id: String,
        #[serde(default = "default_pane_lines")]
        lines: usize,
    },
    /// Read the progress file for a session.
    ReadProgress {
        id: String,
    },

    // ── Status ────────────────────────────────────────────────────
    Status,

    // ── Batch ─────────────────────────────────────────────────────
    Batch {
        ops: Vec<ControlOp>,
    },
}

fn default_pane_lines() -> usize {
    30
}

/// Response returned to the external caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok {
        message: String,
    },
    Session {
        session: SessionInfo,
    },
    SessionList {
        sessions: Vec<SessionInfo>,
    },
    GroupList {
        groups: Vec<GroupInfo>,
    },
    RelationshipList {
        relationships: Vec<RelationshipInfo>,
    },
    /// Raw text content (pane output, progress file, etc.)
    TextContent {
        content: String,
    },
    StatusReport {
        total: usize,
        running: usize,
        waiting: usize,
        idle: usize,
        error: usize,
    },
    BatchResult {
        results: Vec<ControlResponse>,
    },
    Error {
        message: String,
    },
}

/// Serializable session info (external API surface).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub project_path: String,
    pub group_path: String,
    pub status: String,
    pub label: String,
    pub label_color: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

impl SessionInfo {
    pub fn from_instance(inst: &crate::session::Instance) -> Self {
        Self {
            id: inst.id.clone(),
            title: inst.title.clone(),
            project_path: inst.project_path.to_string_lossy().to_string(),
            group_path: inst.group_path.clone(),
            status: format!("{:?}", inst.status).to_lowercase(),
            label: inst.label.clone(),
            label_color: format!("{:?}", inst.label_color).to_lowercase(),
            tags: inst.tags.clone(),
            command: if inst.command.is_empty() {
                None
            } else {
                Some(inst.command.clone())
            },
            tool: Some(format!("{:?}", inst.tool).to_lowercase()),
        }
    }
}

/// Serializable group info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub path: String,
    pub name: String,
    pub session_count: usize,
}

/// Serializable relationship info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipInfo {
    pub id: String,
    pub session_a: String,
    pub session_b: String,
    pub relation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub bidirectional: bool,
}

impl RelationshipInfo {
    pub fn from_relationship(rel: &crate::session::Relationship) -> Self {
        Self {
            id: rel.id.clone(),
            session_a: rel.session_a_id.clone(),
            session_b: rel.session_b_id.clone(),
            relation_type: rel.relation_type.to_string(),
            label: rel.label.clone(),
            bidirectional: rel.bidirectional,
        }
    }
}

/// Helper: parse a relation type string to the enum.
pub fn parse_relation_type(s: &str) -> RelationType {
    match s.to_lowercase().as_str() {
        "parent_child" | "parent-child" => RelationType::ParentChild,
        "peer" => RelationType::Peer,
        "dependency" => RelationType::Dependency,
        "collaboration" => RelationType::Collaboration,
        _ => RelationType::Custom,
    }
}
