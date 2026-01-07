//! Activity analytics - tracks user interactions with sessions
//!
//! Records events like:
//! - Session enter (attach)
//! - Session exit (detach via Ctrl+Q)
//! - Switcher usage
//!
//! Enable in config.json: { "analytics": { "enabled": true } }

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::Result;
use crate::session::Storage;

/// Type of activity event
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// User entered/attached to a session
    Enter,
    /// User exited/detached from a session (Ctrl+Q)
    Exit,
    /// User used the switcher popup
    Switch,
}

/// A single activity event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub session_id: String,
    pub session_name: String,
    /// Duration in seconds (for Exit events, time since last Enter)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
}

/// Daily activity log
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyLog {
    pub date: String, // YYYY-MM-DD
    pub events: Vec<ActivityEvent>,
}

/// Activity tracker
pub struct ActivityTracker {
    enabled: bool,
    profile: String,
    /// Last enter timestamp per session (for calculating duration)
    last_enter: std::collections::HashMap<String, DateTime<Utc>>,
}

impl ActivityTracker {
    /// Create a new tracker (checks config for enabled state)
    pub async fn new(profile: &str) -> Self {
        let enabled = crate::config::ConfigFile::load()
            .await
            .ok()
            .flatten()
            .map(|c| c.analytics_enabled())
            .unwrap_or(false);

        Self {
            enabled,
            profile: profile.to_string(),
            last_enter: std::collections::HashMap::new(),
        }
    }

    /// Check if analytics is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the log file path for today
    fn log_path(&self) -> Result<PathBuf> {
        let base = Storage::get_agent_hand_dir()?;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        Ok(base
            .join("profiles")
            .join(&self.profile)
            .join("analytics")
            .join(format!("{}.json", date)))
    }

    /// Record a session enter event
    pub async fn record_enter(&mut self, session_id: &str, session_name: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let now = Utc::now();
        self.last_enter.insert(session_id.to_string(), now);

        let event = ActivityEvent {
            timestamp: now,
            event_type: EventType::Enter,
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            duration_secs: None,
        };

        self.append_event(event).await
    }

    /// Record a session exit event
    pub async fn record_exit(&mut self, session_id: &str, session_name: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let now = Utc::now();
        let duration = self
            .last_enter
            .remove(session_id)
            .map(|enter| (now - enter).num_seconds().max(0) as u64);

        let event = ActivityEvent {
            timestamp: now,
            event_type: EventType::Exit,
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            duration_secs: duration,
        };

        self.append_event(event).await
    }

    /// Record a switcher usage event
    pub async fn record_switch(&mut self, session_id: &str, session_name: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let event = ActivityEvent {
            timestamp: Utc::now(),
            event_type: EventType::Switch,
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            duration_secs: None,
        };

        self.append_event(event).await
    }

    /// Append an event to today's log file
    async fn append_event(&self, event: ActivityEvent) -> Result<()> {
        let path = self.log_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Load existing log or create new
        let mut log = if path.exists() {
            let content = fs::read_to_string(&path).await?;
            serde_json::from_str::<DailyLog>(&content).unwrap_or_default()
        } else {
            DailyLog {
                date: Utc::now().format("%Y-%m-%d").to_string(),
                events: Vec::new(),
            }
        };

        log.events.push(event);

        // Write back
        let json = serde_json::to_string_pretty(&log)?;
        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(json.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&temp_path, &path).await?;

        Ok(())
    }

    /// Load today's activity log
    pub async fn load_today(&self) -> Result<DailyLog> {
        if !self.enabled {
            return Ok(DailyLog::default());
        }

        let path = self.log_path()?;
        if !path.exists() {
            return Ok(DailyLog {
                date: Utc::now().format("%Y-%m-%d").to_string(),
                events: Vec::new(),
            });
        }

        let content = fs::read_to_string(&path).await?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Get a summary of today's activity
    pub async fn today_summary(&self) -> Result<ActivitySummary> {
        let log = self.load_today().await?;
        Ok(ActivitySummary::from_log(&log))
    }
}

/// Summary of activity for a time period
#[derive(Debug, Clone, Default)]
pub struct ActivitySummary {
    pub total_enters: u32,
    pub total_exits: u32,
    pub total_switches: u32,
    pub total_duration_secs: u64,
    pub sessions_touched: Vec<String>,
}

impl ActivitySummary {
    pub fn from_log(log: &DailyLog) -> Self {
        let mut summary = Self::default();
        let mut sessions = std::collections::HashSet::new();

        for event in &log.events {
            sessions.insert(event.session_name.clone());
            match event.event_type {
                EventType::Enter => summary.total_enters += 1,
                EventType::Exit => {
                    summary.total_exits += 1;
                    if let Some(d) = event.duration_secs {
                        summary.total_duration_secs += d;
                    }
                }
                EventType::Switch => summary.total_switches += 1,
            }
        }

        summary.sessions_touched = sessions.into_iter().collect();
        summary.sessions_touched.sort();
        summary
    }

    /// Format duration as human-readable string
    pub fn format_duration(&self) -> String {
        let hours = self.total_duration_secs / 3600;
        let mins = (self.total_duration_secs % 3600) / 60;
        if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}
