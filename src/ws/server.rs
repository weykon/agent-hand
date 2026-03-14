use std::collections::HashSet;
use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, info, warn};

use super::{
    BroadcastUpdate, Channel, WsClientMessage, WsConfig, WsRequest, WsRequestMsg,
    WsServerMessage,
};

/// WebSocket server handle. Holds a JoinHandle to the background accept loop.
pub struct WsServer {
    _handle: tokio::task::JoinHandle<()>,
    pub addr: SocketAddr,
}

impl WsServer {
    /// Start the WebSocket server. Returns:
    /// - `mpsc::UnboundedReceiver<WsRequestMsg>`: request/response channel for the App event loop
    /// - `Self`: server handle (keeps the accept loop alive)
    pub async fn start(
        config: &WsConfig,
        broadcast_tx: broadcast::Sender<BroadcastUpdate>,
    ) -> std::io::Result<(mpsc::UnboundedReceiver<WsRequestMsg>, Self)> {
        let (request_tx, request_rx) = mpsc::unbounded_channel::<WsRequestMsg>();

        let addr: SocketAddr = format!("{}:{}", config.bind, config.port)
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let listener = TcpListener::bind(addr).await?;
        let bound_addr = listener.local_addr()?;
        info!("WebSocket server listening on ws://{}", bound_addr);

        let handle = tokio::spawn(accept_loop(listener, broadcast_tx, request_tx));

        Ok((
            request_rx,
            Self {
                _handle: handle,
                addr: bound_addr,
            },
        ))
    }
}

/// Accept loop: listen for TCP connections and upgrade to WebSocket.
async fn accept_loop(
    listener: TcpListener,
    broadcast_tx: broadcast::Sender<BroadcastUpdate>,
    request_tx: mpsc::UnboundedSender<WsRequestMsg>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                debug!("WS: new TCP connection from {addr}");
                let broadcast_rx = broadcast_tx.subscribe();
                let req_tx = request_tx.clone();
                tokio::spawn(handle_connection(stream, addr, broadcast_rx, req_tx));
            }
            Err(e) => {
                warn!("WS accept error: {e}");
            }
        }
    }
}

/// Handle a single WebSocket connection.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    mut broadcast_rx: broadcast::Receiver<BroadcastUpdate>,
    request_tx: mpsc::UnboundedSender<WsRequestMsg>,
) {
    // Upgrade TCP → WebSocket
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("WS handshake failed for {addr}: {e}");
            return;
        }
    };

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // Send Welcome message
    let welcome = WsServerMessage::Welcome {
        channels: vec![
            Channel::Canvas,
            Channel::Sessions,
            Channel::Groups,
            Channel::Relationships,
        ],
        version: crate::VERSION.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = ws_write
            .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
            .await;
    }

    info!("WS: client connected from {addr}");

    // Per-connection subscribed channels
    let mut subscribed: HashSet<Channel> = HashSet::new();

    loop {
        tokio::select! {
            // Read from client
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        let text: &str = &text;
                        match serde_json::from_str::<WsClientMessage>(text) {
                            Ok(WsClientMessage::Subscribe { channels }) => {
                                for ch in channels {
                                    subscribed.insert(ch);
                                }
                                debug!("WS {addr}: subscribed to {:?}", subscribed);
                            }
                            Ok(WsClientMessage::Unsubscribe { channels }) => {
                                for ch in &channels {
                                    subscribed.remove(ch);
                                }
                                debug!("WS {addr}: unsubscribed, now {:?}", subscribed);
                            }
                            Ok(WsClientMessage::Request { id, op }) => {
                                let response = handle_request(op, &request_tx).await;
                                let msg = WsServerMessage::Response { id, data: response };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    if ws_write.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                let msg = WsServerMessage::Error {
                                    message: format!("invalid message: {e}"),
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = ws_write.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await;
                                }
                            }
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => {
                        break;
                    }
                    Some(Err(e)) => {
                        debug!("WS {addr}: read error: {e}");
                        break;
                    }
                    _ => {
                        // Ping/Pong/Binary — ignore
                    }
                }
            }

            // Broadcast updates → push to client if subscribed
            Ok(update) = broadcast_rx.recv() => {
                if subscribed.contains(&update.channel) {
                    let msg = WsServerMessage::Push {
                        channel: update.channel,
                        data: update.data,
                    };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        if ws_write.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    info!("WS: client disconnected from {addr}");
}

/// Forward a request to the App event loop and await the response.
async fn handle_request(
    op: WsRequest,
    request_tx: &mpsc::UnboundedSender<WsRequestMsg>,
) -> serde_json::Value {
    let (reply_tx, reply_rx) = oneshot::channel();

    if request_tx.send((op, reply_tx)).is_err() {
        return serde_json::json!({ "error": "server unavailable" });
    }

    match reply_rx.await {
        Ok(value) => value,
        Err(_) => serde_json::json!({ "error": "request dropped" }),
    }
}
