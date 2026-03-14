use std::path::PathBuf;

use tokio::sync::{broadcast, mpsc};

use super::event::HookEvent;

/// Unix domain socket server for receiving hook events from `agent-hand-bridge`.
///
/// External hook binaries connect, write a single JSON line (a serialised `HookEvent`),
/// and disconnect. The server forwards parsed events via an unbounded mpsc channel
/// (for the main event loop) and a broadcast channel (for background subscribers like
/// the sound notification task).
///
/// On non-Unix platforms, `start()` returns a dummy receiver (no socket is opened).
pub struct HookSocketServer {
    socket_path: PathBuf,
    broadcast_tx: broadcast::Sender<HookEvent>,
}

impl HookSocketServer {
    /// Default socket path: `~/.agent-hand/events/hook.sock`
    pub fn default_socket_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agent-hand")
            .join("events")
            .join("hook.sock")
    }

    /// Start the socket server. Returns the receiver end for the event loop
    /// and the server handle (which cleans up the socket file on drop).
    pub fn start() -> (mpsc::UnboundedReceiver<HookEvent>, Self) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(256);
        let socket_path = Self::default_socket_path();
        let server = Self { socket_path, broadcast_tx };
        #[cfg(unix)]
        server.spawn_listener(tx);
        #[cfg(not(unix))]
        {
            tracing::debug!("Hook socket not available on this platform");
            let _ = tx;
        }
        (rx, server)
    }

    /// Subscribe to hook events via the broadcast channel.
    /// Used by background tasks (e.g. sound notifications) that need events
    /// independently from the main event loop.
    pub fn subscribe(&self) -> broadcast::Receiver<HookEvent> {
        self.broadcast_tx.subscribe()
    }

    #[cfg(unix)]
    fn spawn_listener(&self, tx: mpsc::UnboundedSender<HookEvent>) {
        use tokio::io::AsyncBufReadExt;
        use tokio::net::UnixListener;
        use tracing::{debug, error, info, warn};

        let path = self.socket_path.clone();
        let broadcast_tx = self.broadcast_tx.clone();

        tokio::spawn(async move {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            // Remove stale socket file (from previous crash)
            let _ = tokio::fs::remove_file(&path).await;

            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind hook socket at {}: {e}", path.display());
                    return;
                }
            };

            // Make socket accessible to the hook binary
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660));
            }

            info!("Hook socket listening on {}", path.display());

            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        debug!("Hook socket: new connection");
                        let tx = tx.clone();
                        let broadcast_tx = broadcast_tx.clone();
                        tokio::spawn(async move {
                            let reader = tokio::io::BufReader::new(stream);
                            let mut lines = reader.lines();

                            while let Ok(Some(line)) = lines.next_line().await {
                                let line = line.trim().to_string();
                                if line.is_empty() {
                                    continue;
                                }

                                match serde_json::from_str::<HookEvent>(&line) {
                                    Ok(event) => {
                                        debug!("Hook socket: received {:?} from {}", event.kind, event.tmux_session);
                                        // Send to broadcast (sound task etc.) — ignore error
                                        // (no subscribers is fine)
                                        let _ = broadcast_tx.send(event.clone());
                                        // Send to mpsc (main event loop)
                                        if tx.send(event).is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Hook socket: failed to parse event: {e} — line: {line}");
                                    }
                                }
                            }

                            debug!("Hook socket: connection closed");
                        });
                    }
                    Err(e) => {
                        warn!("Hook socket accept error: {e}");
                    }
                }
            }
        });
    }

    /// Get a clone of the broadcast sender so callers can forward events
    /// from other sources (e.g. JSONL fallback) to the broadcast channel.
    pub fn broadcast_tx(&self) -> broadcast::Sender<HookEvent> {
        self.broadcast_tx.clone()
    }
}

impl Drop for HookSocketServer {
    fn drop(&mut self) {
        // Best-effort cleanup of socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
