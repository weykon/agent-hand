//! Session log rotation and compression
//!
//! Only compiled with `input-logging` feature

use std::path::PathBuf;
use tokio::fs;

use crate::config::InputLoggingConfig;
use crate::error::Result;

/// Rotate and compress session logs
pub struct LogRotator {
    log_dir: PathBuf,
    config: InputLoggingConfig,
}

impl LogRotator {
    pub fn new(log_dir: PathBuf, config: InputLoggingConfig) -> Self {
        Self { log_dir, config }
    }

    /// Check and rotate logs if needed
    /// Called periodically (e.g., on app startup or session create)
    pub async fn rotate_if_needed(&self) -> Result<()> {
        if !self.log_dir.exists() {
            return Ok(());
        }

        // Find .log files that exceed threshold
        let threshold = self.config.compress_threshold_bytes();
        let mut to_compress: Vec<PathBuf> = Vec::new();

        let mut entries = fs::read_dir(&self.log_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "log").unwrap_or(false) {
                if let Ok(meta) = entry.metadata().await {
                    if meta.len() >= threshold {
                        to_compress.push(path);
                    }
                }
            }
        }

        // Compress each file
        for log_path in to_compress {
            if let Err(e) = self.compress_log(&log_path).await {
                tracing::warn!("Failed to compress {}: {}", log_path.display(), e);
            }
        }

        // Clean up old archives
        self.cleanup_old_archives().await?;

        Ok(())
    }

    /// Compress a single log file to zip
    async fn compress_log(&self, log_path: &PathBuf) -> Result<()> {
        use std::io::{Read, Write};
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;

        let log_path_clone = log_path.clone();
        let log_dir = self.log_dir.clone();

        // Do compression in blocking task
        tokio::task::spawn_blocking(move || -> Result<()> {
            let file_name = log_path_clone
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.log");

            // Generate zip filename with timestamp
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let zip_name = format!(
                "{}_{}.zip",
                file_name.trim_end_matches(".log"),
                timestamp
            );
            let zip_path = log_dir.join(&zip_name);

            // Create zip file
            let zip_file = std::fs::File::create(&zip_path)?;
            let mut zip = ZipWriter::new(zip_file);

            // Read log content
            let mut log_file = std::fs::File::open(&log_path_clone)?;
            let mut content = Vec::new();
            log_file.read_to_end(&mut content)?;

            // Add to zip with compression
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(6));
            zip.start_file(file_name, options)
                .map_err(|e| crate::Error::Other(format!("Zip error: {}", e)))?;
            zip.write_all(&content)?;
            zip.finish()
                .map_err(|e| crate::Error::Other(format!("Zip finish error: {}", e)))?;

            // Remove original log file
            std::fs::remove_file(&log_path_clone)?;

            tracing::info!(
                "Compressed {} -> {} ({} bytes)",
                file_name,
                zip_name,
                content.len()
            );

            Ok(())
        })
        .await
        .map_err(|e| crate::Error::Other(format!("Compression task failed: {}", e)))??;

        Ok(())
    }

    /// Remove oldest archives if exceeding max_archives
    async fn cleanup_old_archives(&self) -> Result<()> {
        let mut archives: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        let mut entries = fs::read_dir(&self.log_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "zip").unwrap_or(false) {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        archives.push((path, modified));
                    }
                }
            }
        }

        // Sort by modified time (oldest first)
        archives.sort_by(|a, b| a.1.cmp(&b.1));

        // Remove oldest if exceeding limit
        let to_remove = archives.len().saturating_sub(self.config.max_archives);
        for (path, _) in archives.into_iter().take(to_remove) {
            if let Err(e) = fs::remove_file(&path).await {
                tracing::warn!("Failed to remove old archive {}: {}", path.display(), e);
            } else {
                tracing::info!("Removed old archive: {}", path.display());
            }
        }

        Ok(())
    }
}

/// Get the session logs directory for a profile
pub fn get_session_logs_dir(profile: &str) -> Result<PathBuf> {
    let base = crate::session::Storage::get_agent_hand_dir()?;
    Ok(base.join("profiles").join(profile).join("session-logs"))
}
