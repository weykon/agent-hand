use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use crate::error::Result;

use super::event::HookEvent;

/// Reads hook events from a JSONL file, tracking position across calls.
///
/// Each call to `poll()` returns any new events appended since the last read.
/// The file is `~/.agent-hand/events/hook-events.jsonl`.
pub struct EventReceiver {
    path: PathBuf,
    /// Byte offset of the last read position
    offset: u64,
}

impl EventReceiver {
    /// Create a new receiver. Starts reading from the end of the file
    /// (ignores events from before this process started).
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| crate::Error::config("Cannot determine home directory"))?;
        let path = home.join(".agent-hand/events/hook-events.jsonl");

        // Start from end of file (if it exists) so we only see new events
        let offset = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(Self { path, offset })
    }

    /// Poll for new events since last read. Returns empty vec if no new events.
    /// This is cheap to call frequently — it's just a file stat + read if changed.
    pub fn poll(&mut self) -> Vec<HookEvent> {
        let mut events = Vec::new();

        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return events, // File doesn't exist yet — no events
        };

        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(_) => return events,
        };

        let file_len = metadata.len();

        // File was truncated or rotated — reset offset
        if file_len < self.offset {
            self.offset = 0;
        }

        // No new data
        if file_len == self.offset {
            return events;
        }

        // Seek to last known position and read new lines
        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(self.offset)).is_err() {
            return events;
        }

        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    self.offset += n as u64;
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<HookEvent>(trimmed) {
                        Ok(event) => events.push(event),
                        Err(e) => {
                            tracing::warn!("Failed to parse hook event: {} — line: {}", e, trimmed);
                        }
                    }
                }
                Err(_) => break,
            }
        }

        events
    }

    /// Get the events file path (for hook scripts to write to).
    pub fn events_file_path(&self) -> &PathBuf {
        &self.path
    }
}
