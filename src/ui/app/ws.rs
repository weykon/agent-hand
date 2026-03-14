use super::*;
use crate::control::{GroupInfo, RelationshipInfo, SessionInfo};
use crate::ws::{BroadcastUpdate, Channel, WsRequest};

impl App {
    /// Handle a read-only WebSocket request from an external client.
    pub(super) fn handle_ws_request(&self, req: WsRequest) -> serde_json::Value {
        match req {
            WsRequest::QueryCanvas { what } => {
                let response = self.canvas_state.query(&what, None, None, None);
                serde_json::to_value(&response).unwrap_or_default()
            }
            WsRequest::ListSessions => {
                let sessions: Vec<SessionInfo> = self
                    .sessions
                    .iter()
                    .map(SessionInfo::from_instance)
                    .collect();
                serde_json::to_value(&sessions).unwrap_or_default()
            }
            WsRequest::ListGroups => {
                let groups: Vec<GroupInfo> = self
                    .groups
                    .all_groups()
                    .into_iter()
                    .map(|g| {
                        let session_count = self
                            .sessions
                            .iter()
                            .filter(|s| {
                                s.group_path == g.path
                                    || s.group_path.starts_with(&format!("{}/", g.path))
                            })
                            .count();
                        GroupInfo {
                            path: g.path,
                            name: g.name,
                            session_count,
                        }
                    })
                    .collect();
                serde_json::to_value(&groups).unwrap_or_default()
            }
            WsRequest::Status => {
                let total = self.sessions.len();
                let running = self
                    .sessions
                    .iter()
                    .filter(|s| matches!(s.status, Status::Running))
                    .count();
                let waiting = self
                    .sessions
                    .iter()
                    .filter(|s| matches!(s.status, Status::Waiting))
                    .count();
                let idle = self
                    .sessions
                    .iter()
                    .filter(|s| matches!(s.status, Status::Idle))
                    .count();
                let error = self
                    .sessions
                    .iter()
                    .filter(|s| matches!(s.status, Status::Error))
                    .count();
                serde_json::json!({
                    "total": total,
                    "running": running,
                    "waiting": waiting,
                    "idle": idle,
                    "error": error,
                })
            }
        }
    }

    /// Hash-based change detection: broadcast updates for any state that changed since last call.
    pub(super) fn broadcast_changes(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Canvas state hash (via node/edge count + positions as a cheap proxy)
        let new_canvas_hash = {
            let mut h = DefaultHasher::new();
            self.canvas_state.graph.node_count().hash(&mut h);
            self.canvas_state.graph.edge_count().hash(&mut h);
            // Include positions for move detection
            for (&idx, &(x, y)) in &self.canvas_state.positions {
                idx.index().hash(&mut h);
                x.hash(&mut h);
                y.hash(&mut h);
            }
            h.finish()
        };
        if new_canvas_hash != self.max.ws_canvas_hash {
            self.max.ws_canvas_hash = new_canvas_hash;
            let response = self.canvas_state.query("state", None, None, None);
            if let Ok(data) = serde_json::to_value(&response) {
                let _ = self.max.ws_broadcast_tx.send(BroadcastUpdate {
                    channel: Channel::Canvas,
                    data,
                });
            }
        }

        // Sessions hash
        let new_sessions_hash = {
            let mut h = DefaultHasher::new();
            for s in &self.sessions {
                s.id.hash(&mut h);
                s.title.hash(&mut h);
                format!("{:?}", s.status).hash(&mut h);
                s.group_path.hash(&mut h);
                s.label.hash(&mut h);
                s.tags.hash(&mut h);
            }
            h.finish()
        };
        if new_sessions_hash != self.max.ws_sessions_hash {
            self.max.ws_sessions_hash = new_sessions_hash;
            let sessions: Vec<SessionInfo> = self
                .sessions
                .iter()
                .map(SessionInfo::from_instance)
                .collect();
            if let Ok(data) = serde_json::to_value(&sessions) {
                let _ = self.max.ws_broadcast_tx.send(BroadcastUpdate {
                    channel: Channel::Sessions,
                    data,
                });
            }
        }

        // Groups hash
        let new_groups_hash = {
            let mut h = DefaultHasher::new();
            for g in self.groups.all_groups() {
                g.path.hash(&mut h);
                g.name.hash(&mut h);
            }
            self.sessions.len().hash(&mut h);
            h.finish()
        };
        if new_groups_hash != self.max.ws_groups_hash {
            self.max.ws_groups_hash = new_groups_hash;
            let groups: Vec<GroupInfo> = self
                .groups
                .all_groups()
                .into_iter()
                .map(|g| {
                    let session_count = self
                        .sessions
                        .iter()
                        .filter(|s| {
                            s.group_path == g.path
                                || s.group_path.starts_with(&format!("{}/", g.path))
                        })
                        .count();
                    GroupInfo {
                        path: g.path,
                        name: g.name,
                        session_count,
                    }
                })
                .collect();
            if let Ok(data) = serde_json::to_value(&groups) {
                let _ = self.max.ws_broadcast_tx.send(BroadcastUpdate {
                    channel: Channel::Groups,
                    data,
                });
            }
        }

        // Relationships hash
        let new_rels_hash = {
            let mut h = DefaultHasher::new();
            for r in &self.relationships {
                r.id.hash(&mut h);
                r.session_a_id.hash(&mut h);
                r.session_b_id.hash(&mut h);
            }
            h.finish()
        };
        if new_rels_hash != self.max.ws_relationships_hash {
            self.max.ws_relationships_hash = new_rels_hash;
            let rels: Vec<RelationshipInfo> = self
                .relationships
                .iter()
                .map(RelationshipInfo::from_relationship)
                .collect();
            if let Ok(data) = serde_json::to_value(&rels) {
                let _ = self.max.ws_broadcast_tx.send(BroadcastUpdate {
                    channel: Channel::Relationships,
                    data,
                });
            }
        }
    }
}
