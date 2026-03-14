use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of relationship between two sessions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// A spawned B (existing fork mechanism)
    ParentChild,
    /// A and B work on related parts (bidirectional)
    Peer,
    /// A depends on B's output (A→B)
    Dependency,
    /// A and B actively collaborate (bidirectional)
    Collaboration,
    /// User-defined relationship
    Custom,
}

impl RelationType {
    /// Whether this relationship type is inherently bidirectional
    pub fn is_bidirectional(&self) -> bool {
        matches!(self, Self::Peer | Self::Collaboration | Self::Custom)
    }
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParentChild => write!(f, "parent-child"),
            Self::Peer => write!(f, "peer"),
            Self::Dependency => write!(f, "dependency"),
            Self::Collaboration => write!(f, "collaboration"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// A relationship between two sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub relation_type: RelationType,
    pub session_a_id: String,
    pub session_b_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub bidirectional: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl Relationship {
    /// Create a new relationship between two sessions
    pub fn new(
        relation_type: RelationType,
        session_a_id: String,
        session_b_id: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..12].to_string(),
            relation_type,
            session_a_id,
            session_b_id,
            label: None,
            created_at: Utc::now(),
            bidirectional: relation_type.is_bidirectional(),
            metadata: HashMap::new(),
        }
    }

    /// Set a label for this relationship
    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    /// Display indicator for TUI: <-> for bidirectional, -> for directional
    pub fn direction_indicator(&self) -> &'static str {
        if self.bidirectional {
            match self.relation_type {
                RelationType::Collaboration => "--",
                _ => "<->",
            }
        } else {
            "->"
        }
    }

    /// Check if a session is part of this relationship
    pub fn involves_session(&self, session_id: &str) -> bool {
        self.session_a_id == session_id || self.session_b_id == session_id
    }

    /// Get the other session in this relationship
    pub fn other_session(&self, session_id: &str) -> Option<&str> {
        if self.session_a_id == session_id {
            Some(&self.session_b_id)
        } else if self.session_b_id == session_id {
            Some(&self.session_a_id)
        } else {
            None
        }
    }
}

// CRUD operations live in the private pro module.
// Re-export them here so callers don't need to change imports.
#[cfg(feature = "pro")]
pub use crate::pro::relationships::{add_relationship, remove_relationship};

pub fn find_relationship<'a>(relationships: &'a [Relationship], id: &str) -> Option<&'a Relationship> {
    relationships.iter().find(|r| r.id == id)
}

pub fn find_relationships_for_session<'a>(
    relationships: &'a [Relationship],
    session_id: &str,
) -> Vec<&'a Relationship> {
    relationships
        .iter()
        .filter(|r| r.involves_session(session_id))
        .collect()
}

/// Find a relationship that connects two specific sessions (in either direction).
pub fn find_relationship_for_sessions<'a>(
    relationships: &'a [Relationship],
    session_a: &str,
    session_b: &str,
) -> Option<&'a Relationship> {
    relationships.iter().find(|r| {
        (r.session_a_id == session_a && r.session_b_id == session_b)
            || (r.session_a_id == session_b && r.session_b_id == session_a)
    })
}
