//! HostRequest executor — processes WASM plugin requests on behalf of the host.
//!
//! Dispatches by request_type:
//! - "cli"       → shell out to allowlisted commands (agent-hand-bridge only)
//! - "api"       → in-process handlers (read_progress, read_cold_memory, query_canvas)
//! - "read_file" → sandboxed file read (relative to runtime_dir, no traversal)
//! - "skill"     → stub (future ACP bridge)

#![cfg(feature = "wasm")]

use std::path::PathBuf;

use super::wasm_canvas::{HostRequest, HostRequestResult};

/// Allowlisted command prefixes for CLI execution.
const CLI_ALLOWLIST: &[&str] = &["agent-hand-bridge"];

/// Maximum stdout capture size (1MB).
const CLI_MAX_OUTPUT: usize = 1_048_576;

/// Maximum file read size (512KB).
const FILE_MAX_SIZE: u64 = 524_288;

/// CLI command timeout in seconds.
const CLI_TIMEOUT_SECS: u64 = 10;

/// Executes host requests from WASM plugins.
///
/// Each request is dispatched by its `request_type` field. Failures are
/// non-fatal — the executor returns an error result to the plugin rather
/// than panicking or propagating.
pub struct HostRequestExecutor {
    runtime_dir: PathBuf,
    progress_dir: PathBuf,
}

impl HostRequestExecutor {
    pub fn new(runtime_dir: PathBuf, progress_dir: PathBuf) -> Self {
        Self {
            runtime_dir,
            progress_dir,
        }
    }

    /// Execute all pending requests and collect results.
    pub async fn execute_all(&self, requests: Vec<HostRequest>) -> Vec<HostRequestResult> {
        let mut results = Vec::with_capacity(requests.len());
        for req in &requests {
            let result = match req.request_type.as_str() {
                "cli" => self.execute_cli(req).await,
                "api" => self.execute_api(req),
                "read_file" => self.execute_read_file(req),
                "skill" => self.execute_skill(req),
                other => HostRequestResult {
                    request_id: req.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some(format!("unknown request_type: {}", other)),
                },
            };
            results.push(result);
        }
        results
    }

    /// Execute a CLI command (allowlisted only).
    async fn execute_cli(&self, req: &HostRequest) -> HostRequestResult {
        let target = &req.target;

        // Check allowlist
        let allowed = CLI_ALLOWLIST
            .iter()
            .any(|prefix| target.starts_with(prefix));
        if !allowed {
            return HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some(format!(
                    "command not allowed: '{}' (only {:?} permitted)",
                    target, CLI_ALLOWLIST
                )),
            };
        }

        // Build command
        let parts: Vec<&str> = target.split_whitespace().collect();
        if parts.is_empty() {
            return HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some("empty command".to_string()),
            };
        }

        let mut cmd = tokio::process::Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }
        // Append extra args from the request
        for (key, value) in &req.args {
            cmd.arg(format!("--{}", key));
            if !value.is_empty() {
                cmd.arg(value);
            }
        }

        // Execute with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(CLI_TIMEOUT_SECS),
            cmd.output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Cap output size
                let data_str = if stdout.len() > CLI_MAX_OUTPUT {
                    &stdout[..CLI_MAX_OUTPUT]
                } else {
                    &stdout
                };

                if output.status.success() {
                    HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: true,
                        data: Some(serde_json::Value::String(data_str.to_string())),
                        error: None,
                    }
                } else {
                    HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: false,
                        data: Some(serde_json::Value::String(data_str.to_string())),
                        error: Some(format!("exit code {}: {}", output.status, stderr.trim())),
                    }
                }
            }
            Ok(Err(e)) => HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some(format!("command failed: {}", e)),
            },
            Err(_) => HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some(format!("command timed out after {}s", CLI_TIMEOUT_SECS)),
            },
        }
    }

    /// Execute an in-process API request.
    fn execute_api(&self, req: &HostRequest) -> HostRequestResult {
        match req.target.as_str() {
            "read_progress" => {
                let session_key = req
                    .args
                    .get("session_key")
                    .cloned()
                    .unwrap_or_default();
                if session_key.is_empty() {
                    return HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some("missing 'session_key' arg".to_string()),
                    };
                }
                let path = self.progress_dir.join(format!("{}.md", session_key));
                match std::fs::read_to_string(&path) {
                    Ok(content) => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: true,
                        data: Some(serde_json::Value::String(content)),
                        error: None,
                    },
                    Err(e) => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(format!("read_progress: {}", e)),
                    },
                }
            }
            "read_cold_memory" => {
                let path = self.runtime_dir.join("cold_memory_snapshot.json");
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let value: serde_json::Value =
                            serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
                        HostRequestResult {
                            request_id: req.request_id.clone(),
                            success: true,
                            data: Some(value),
                            error: None,
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: true,
                        data: Some(serde_json::json!([])),
                        error: None,
                    },
                    Err(e) => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(format!("read_cold_memory: {}", e)),
                    },
                }
            }
            "query_canvas" => {
                let path = self.runtime_dir.join("wasm_canvas_ops.json");
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let value: serde_json::Value =
                            serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
                        HostRequestResult {
                            request_id: req.request_id.clone(),
                            success: true,
                            data: Some(value),
                            error: None,
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: true,
                        data: Some(serde_json::json!([])),
                        error: None,
                    },
                    Err(e) => HostRequestResult {
                        request_id: req.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(format!("query_canvas: {}", e)),
                    },
                }
            }
            other => HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some(format!("unknown api endpoint: {}", other)),
            },
        }
    }

    /// Read a file relative to runtime_dir (sandboxed).
    fn execute_read_file(&self, req: &HostRequest) -> HostRequestResult {
        let relative = &req.target;

        // Security: reject path traversal
        if relative.contains("..") || relative.starts_with('/') || relative.starts_with('\\') {
            return HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some("path traversal not allowed".to_string()),
            };
        }

        let path = self.runtime_dir.join(relative);

        // Verify resolved path is under runtime_dir
        match path.canonicalize() {
            Ok(canonical) => {
                if let Ok(runtime_canonical) = self.runtime_dir.canonicalize() {
                    if !canonical.starts_with(&runtime_canonical) {
                        return HostRequestResult {
                            request_id: req.request_id.clone(),
                            success: false,
                            data: None,
                            error: Some("path escapes runtime directory".to_string()),
                        };
                    }
                }
            }
            Err(e) => {
                return HostRequestResult {
                    request_id: req.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some(format!("file not found: {}", e)),
                };
            }
        }

        // Check size
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > FILE_MAX_SIZE => {
                return HostRequestResult {
                    request_id: req.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some(format!(
                        "file too large: {} bytes (max {})",
                        meta.len(),
                        FILE_MAX_SIZE
                    )),
                };
            }
            Err(e) => {
                return HostRequestResult {
                    request_id: req.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some(format!("cannot stat file: {}", e)),
                };
            }
            _ => {}
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => HostRequestResult {
                request_id: req.request_id.clone(),
                success: true,
                data: Some(serde_json::Value::String(content)),
                error: None,
            },
            Err(e) => HostRequestResult {
                request_id: req.request_id.clone(),
                success: false,
                data: None,
                error: Some(format!("read error: {}", e)),
            },
        }
    }

    /// Skill execution (stub — future ACP bridge).
    fn execute_skill(&self, req: &HostRequest) -> HostRequestResult {
        HostRequestResult {
            request_id: req.request_id.clone(),
            success: false,
            data: None,
            error: Some(format!(
                "skill execution not yet supported: '{}'",
                req.target
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_executor(tmp: &TempDir) -> HostRequestExecutor {
        let runtime = tmp.path().join("runtime");
        let progress = tmp.path().join("progress");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::create_dir_all(&progress).unwrap();
        HostRequestExecutor::new(runtime, progress)
    }

    fn make_request(id: &str, rtype: &str, target: &str) -> HostRequest {
        HostRequest {
            request_id: id.to_string(),
            request_type: rtype.to_string(),
            target: target.to_string(),
            args: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn cli_rejects_disallowed_command() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r1", "cli", "rm -rf /");
        let result = executor.execute_cli(&req).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not allowed"));
    }

    #[tokio::test]
    async fn cli_rejects_empty_command() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r2", "cli", "");
        let result = executor.execute_cli(&req).await;
        assert!(!result.success);
    }

    #[test]
    fn api_read_progress_returns_content() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let progress_file = executor.progress_dir.join("test_session.md");
        std::fs::write(&progress_file, "- [12:00] task.complete\n").unwrap();

        let mut req = make_request("r3", "api", "read_progress");
        req.args
            .insert("session_key".to_string(), "test_session".to_string());
        let result = executor.execute_api(&req);
        assert!(result.success);
        assert!(result
            .data
            .unwrap()
            .as_str()
            .unwrap()
            .contains("task.complete"));
    }

    #[test]
    fn api_read_progress_requires_session_key() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r4", "api", "read_progress");
        let result = executor.execute_api(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("session_key"));
    }

    #[test]
    fn api_read_cold_memory_empty_when_missing() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r5", "api", "read_cold_memory");
        let result = executor.execute_api(&req);
        assert!(result.success);
        assert_eq!(result.data.unwrap(), serde_json::json!([]));
    }

    #[test]
    fn api_read_cold_memory_returns_snapshot() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let snapshot = executor.runtime_dir.join("cold_memory_snapshot.json");
        std::fs::write(&snapshot, r#"[{"key":"value"}]"#).unwrap();

        let req = make_request("r6", "api", "read_cold_memory");
        let result = executor.execute_api(&req);
        assert!(result.success);
        let data = result.data.unwrap();
        assert!(data.is_array());
        assert_eq!(data.as_array().unwrap().len(), 1);
    }

    #[test]
    fn api_unknown_endpoint_fails() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r7", "api", "delete_everything");
        let result = executor.execute_api(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("unknown api"));
    }

    #[test]
    fn read_file_blocks_traversal() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r8", "read_file", "../../etc/passwd");
        let result = executor.execute_read_file(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("traversal"));
    }

    #[test]
    fn read_file_blocks_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r9", "read_file", "/etc/passwd");
        let result = executor.execute_read_file(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("traversal"));
    }

    #[test]
    fn read_file_succeeds_for_valid_path() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let test_file = executor.runtime_dir.join("test.json");
        std::fs::write(&test_file, r#"{"hello":"world"}"#).unwrap();

        let req = make_request("r10", "read_file", "test.json");
        let result = executor.execute_read_file(&req);
        assert!(result.success);
        assert!(result.data.unwrap().as_str().unwrap().contains("hello"));
    }

    #[test]
    fn read_file_caps_large_files() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let big_file = executor.runtime_dir.join("big.bin");
        // Write a file larger than FILE_MAX_SIZE
        let data = vec![b'x'; (FILE_MAX_SIZE + 1) as usize];
        std::fs::write(&big_file, &data).unwrap();

        let req = make_request("r11", "read_file", "big.bin");
        let result = executor.execute_read_file(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("too large"));
    }

    #[test]
    fn skill_execution_returns_stub_error() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);
        let req = make_request("r12", "skill", "canvas-ops");
        let result = executor.execute_skill(&req);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not yet supported"));
    }

    #[tokio::test]
    async fn execute_all_dispatches_by_type() {
        let tmp = TempDir::new().unwrap();
        let executor = make_executor(&tmp);

        // Write a test file for the read_file request
        let test_file = executor.runtime_dir.join("data.txt");
        std::fs::write(&test_file, "content").unwrap();

        let requests = vec![
            make_request("a", "read_file", "data.txt"),
            make_request("b", "api", "read_cold_memory"),
            make_request("c", "skill", "canvas-ops"),
            make_request("d", "unknown_type", "whatever"),
        ];

        let results = executor.execute_all(requests).await;
        assert_eq!(results.len(), 4);
        assert!(results[0].success, "read_file should succeed");
        assert!(results[1].success, "read_cold_memory should succeed");
        assert!(!results[2].success, "skill should fail (stub)");
        assert!(!results[3].success, "unknown type should fail");
    }
}
