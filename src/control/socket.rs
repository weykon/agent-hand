use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use super::{ControlOp, ControlResponse};

/// A request from a socket client: an op paired with a reply channel.
pub type ControlRequest = (ControlOp, oneshot::Sender<ControlResponse>);

/// Unix domain socket server for external control operations (session/group/tag management).
///
/// External tools send JSON lines (one `ControlOp` per line) and receive
/// a `ControlResponse` JSON line back for each.
///
/// On non-Unix platforms, `start()` returns a dummy receiver (no socket is opened).
pub struct ControlSocketServer {
    _op_tx: mpsc::UnboundedSender<ControlRequest>,
    socket_path: PathBuf,
}

impl ControlSocketServer {
    /// Returns the default socket path: `~/.agent-hand/control.sock`
    pub fn default_socket_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agent-hand")
            .join("control.sock")
    }

    /// Start the socket server. Returns the receiver end for the event loop.
    pub fn start() -> (mpsc::UnboundedReceiver<ControlRequest>, Self) {
        let (op_tx, op_rx) = mpsc::unbounded_channel();
        let socket_path = Self::default_socket_path();
        let server = Self {
            _op_tx: op_tx.clone(),
            socket_path,
        };
        #[cfg(unix)]
        server.spawn_listener(op_tx);
        #[cfg(not(unix))]
        tracing::debug!("Control socket not available on this platform");
        let _ = op_tx;
        (op_rx, server)
    }

    #[cfg(unix)]
    fn spawn_listener(&self, tx: mpsc::UnboundedSender<ControlRequest>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;
        use tracing::{debug, error, info, warn};

        let path = self.socket_path.clone();

        tokio::spawn(async move {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            // Remove stale socket file
            let _ = tokio::fs::remove_file(&path).await;

            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    error!(
                        "Failed to bind control socket at {}: {e}",
                        path.display()
                    );
                    return;
                }
            };

            // Make socket group-readable/writable so other tools can connect
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660));
            }

            info!("Control socket listening on {}", path.display());

            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        debug!("Control socket: new connection");
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            let (reader, mut writer) = stream.into_split();
                            let mut lines = BufReader::new(reader).lines();

                            while let Ok(Some(line)) = lines.next_line().await {
                                let line = line.trim().to_string();
                                if line.is_empty() {
                                    continue;
                                }

                                let op: ControlOp = match serde_json::from_str(&line) {
                                    Ok(op) => op,
                                    Err(e) => {
                                        let err = ControlResponse::Error {
                                            message: format!("invalid JSON: {e}"),
                                        };
                                        let mut resp = serde_json::to_string(&err).unwrap_or_default();
                                        resp.push('\n');
                                        let _ = writer.write_all(resp.as_bytes()).await;
                                        continue;
                                    }
                                };

                                let (reply_tx, reply_rx) = oneshot::channel();

                                if tx.send((op, reply_tx)).is_err() {
                                    let err = ControlResponse::Error {
                                        message: "control unavailable".into(),
                                    };
                                    let mut resp = serde_json::to_string(&err).unwrap_or_default();
                                    resp.push('\n');
                                    let _ = writer.write_all(resp.as_bytes()).await;
                                    break;
                                }

                                match reply_rx.await {
                                    Ok(response) => {
                                        let mut resp = serde_json::to_string(&response).unwrap_or_default();
                                        resp.push('\n');
                                        if writer.write_all(resp.as_bytes()).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        break;
                                    }
                                }
                            }

                            debug!("Control socket: connection closed");
                        });
                    }
                    Err(e) => {
                        warn!("Control socket accept error: {e}");
                    }
                }
            }
        });
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

impl Drop for ControlSocketServer {
    fn drop(&mut self) {
        // Best-effort cleanup of socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
