use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use super::{CanvasOp, CanvasResponse};

/// A request from a socket client: an op paired with a reply channel.
pub type CanvasRequest = (CanvasOp, oneshot::Sender<CanvasResponse>);

/// Unix domain socket server for external canvas control.
///
/// External tools send JSON lines (one `CanvasOp` per line) and receive
/// a `CanvasResponse` JSON line back for each.
///
/// On non-Unix platforms, `start()` returns a dummy receiver (no socket is opened).
pub struct CanvasSocketServer {
    #[cfg(unix)]
    op_tx: mpsc::UnboundedSender<CanvasRequest>,
    socket_path: PathBuf,
}

impl CanvasSocketServer {
    /// Returns the default socket path: `~/.agent-hand/canvas.sock`
    pub fn default_socket_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agent-hand")
            .join("canvas.sock")
    }

    /// Start the socket server. Returns the receiver end for the event loop.
    pub fn start() -> (mpsc::UnboundedReceiver<CanvasRequest>, Self) {
        let (op_tx, op_rx) = mpsc::unbounded_channel();
        let socket_path = Self::default_socket_path();
        let server = Self {
            #[cfg(unix)]
            op_tx,
            socket_path,
        };
        #[cfg(unix)]
        server.spawn_listener();
        #[cfg(not(unix))]
        {
            tracing::debug!("Canvas socket not available on this platform");
            let _ = op_tx;
        }
        (op_rx, server)
    }

    #[cfg(unix)]
    fn spawn_listener(&self) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;
        use tracing::{debug, error, info, warn};

        let path = self.socket_path.clone();
        let tx = self.op_tx.clone();

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
                    error!("Failed to bind canvas socket at {}: {e}", path.display());
                    return;
                }
            };

            // Make socket group-readable/writable so other tools can connect
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660));
            }

            info!("Canvas socket listening on {}", path.display());

            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        debug!("Canvas socket: new connection");
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            let (reader, mut writer) = stream.into_split();
                            let mut lines = BufReader::new(reader).lines();

                            while let Ok(Some(line)) = lines.next_line().await {
                                let line = line.trim().to_string();
                                if line.is_empty() {
                                    continue;
                                }

                                let op: CanvasOp = match serde_json::from_str(&line) {
                                    Ok(op) => op,
                                    Err(e) => {
                                        let err = CanvasResponse::Error {
                                            message: format!("invalid JSON: {e}"),
                                        };
                                        let mut resp = serde_json::to_string(&err).unwrap_or_default();
                                        resp.push('\n');
                                        let _ = writer.write_all(resp.as_bytes()).await;
                                        continue;
                                    }
                                };

                                // Validate external ops (prefix + batch size; projection count
                                // checked without state — allows up to MAX_PROJECTION_NODES new).
                                if let Err(reason) = super::validate_external_op(&op, 0) {
                                    let err = CanvasResponse::Error {
                                        message: format!("validation failed: {reason}"),
                                    };
                                    let mut resp = serde_json::to_string(&err).unwrap_or_default();
                                    resp.push('\n');
                                    let _ = writer.write_all(resp.as_bytes()).await;
                                    continue;
                                }

                                let (reply_tx, reply_rx) = oneshot::channel();

                                if tx.send((op, reply_tx)).is_err() {
                                    let err = CanvasResponse::Error {
                                        message: "canvas unavailable".into(),
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

                            debug!("Canvas socket: connection closed");
                        });
                    }
                    Err(e) => {
                        warn!("Canvas socket accept error: {e}");
                    }
                }
            }
        });
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Get a clone of the op sender for pushing canvas ops from other subsystems.
    #[cfg(unix)]
    pub fn op_sender(&self) -> mpsc::UnboundedSender<CanvasRequest> {
        self.op_tx.clone()
    }
}

impl Drop for CanvasSocketServer {
    fn drop(&mut self) {
        // Best-effort cleanup of socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Connect to the canvas socket and send a single op, returning the response.
/// Used by the CLI subcommand. Unix-only.
#[cfg(unix)]
pub async fn send_op(op: &CanvasOp) -> anyhow::Result<CanvasResponse> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let path = CanvasSocketServer::default_socket_path();
    let stream = tokio::net::UnixStream::connect(&path).await
        .map_err(|e| anyhow::anyhow!(
            "Cannot connect to canvas socket at {}: {e}. Is agent-hand running?",
            path.display()
        ))?;

    let (reader, mut writer) = stream.into_split();

    let mut payload = serde_json::to_string(op)?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    writer.shutdown().await?;

    let mut lines = BufReader::new(reader).lines();
    if let Some(line) = lines.next_line().await? {
        let resp: CanvasResponse = serde_json::from_str(&line)?;
        Ok(resp)
    } else {
        Err(anyhow::anyhow!("No response from canvas socket"))
    }
}

/// Stub for non-Unix platforms.
#[cfg(not(unix))]
pub async fn send_op(_op: &CanvasOp) -> anyhow::Result<CanvasResponse> {
    Err(anyhow::anyhow!("Canvas socket is only available on Unix platforms"))
}
