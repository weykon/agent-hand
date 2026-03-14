pub mod server;

use serde::{Deserialize, Serialize};

/// Subscription channel for real-time push updates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Canvas,
    Sessions,
    Groups,
    Relationships,
}

/// Message sent from WebSocket client to server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMessage {
    Subscribe { channels: Vec<Channel> },
    Unsubscribe { channels: Vec<Channel> },
    Request { id: String, op: WsRequest },
}

/// Read-only request operations available over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WsRequest {
    QueryCanvas { what: String },
    ListSessions,
    ListGroups,
    Status,
}

/// Message sent from server to WebSocket client.
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMessage {
    Welcome {
        channels: Vec<Channel>,
        version: String,
    },
    Push {
        channel: Channel,
        data: serde_json::Value,
    },
    Response {
        id: String,
        data: serde_json::Value,
    },
    Error {
        message: String,
    },
}

/// Internal broadcast envelope for pushing updates to all connected clients.
#[derive(Debug, Clone)]
pub struct BroadcastUpdate {
    pub channel: Channel,
    pub data: serde_json::Value,
}

/// Configuration for the WebSocket data transport server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsConfig {
    /// Whether the WebSocket server is enabled.
    #[serde(default = "default_ws_enabled")]
    pub enabled: bool,
    /// TCP port to listen on.
    #[serde(default = "default_ws_port")]
    pub port: u16,
    /// Bind address.
    #[serde(default = "default_ws_bind")]
    pub bind: String,
}

fn default_ws_enabled() -> bool {
    true
}

fn default_ws_port() -> u16 {
    3847
}

fn default_ws_bind() -> String {
    "127.0.0.1".to_string()
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            enabled: default_ws_enabled(),
            port: default_ws_port(),
            bind: default_ws_bind(),
        }
    }
}

/// A request from a WebSocket client paired with a reply channel.
pub type WsRequestMsg = (WsRequest, tokio::sync::oneshot::Sender<serde_json::Value>);
