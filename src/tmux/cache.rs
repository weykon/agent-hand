use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::RwLock;

/// Session cache to reduce tmux subprocess calls
/// Instead of calling `tmux has-session` for each session,
/// we call `tmux list-sessions` ONCE per tick and cache results
#[derive(Debug)]
pub struct SessionCache {
    data: Arc<RwLock<HashMap<String, i64>>>,
    last_update: Arc<RwLock<Option<SystemTime>>>,
    ttl: Duration,
}

impl SessionCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            last_update: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(2), // 2 seconds TTL
        }
    }

    /// Update cache with new session data
    pub fn update(&self, sessions: HashMap<String, i64>) {
        *self.data.write() = sessions;
        *self.last_update.write() = Some(SystemTime::now());
    }

    /// Check if session exists (from cache)
    pub fn exists(&self, name: &str) -> Option<bool> {
        if !self.is_valid() {
            return None; // Cache invalid
        }
        Some(self.data.read().contains_key(name))
    }

    /// Get session activity timestamp (from cache)
    pub fn activity(&self, name: &str) -> Option<i64> {
        if !self.is_valid() {
            return None; // Cache invalid
        }
        self.data.read().get(name).copied()
    }

    /// Register a newly created session
    pub fn register(&self, name: String) {
        let mut data = self.data.write();
        data.insert(
            name,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        );
    }

    /// Check if cache is valid (not expired)
    fn is_valid(&self) -> bool {
        if let Some(last) = *self.last_update.read() {
            SystemTime::now().duration_since(last).unwrap() < self.ttl
        } else {
            false
        }
    }

    /// Clear cache
    pub fn clear(&self) {
        self.data.write().clear();
        *self.last_update.write() = None;
    }
}

impl Default for SessionCache {
    fn default() -> Self {
        Self::new()
    }
}
