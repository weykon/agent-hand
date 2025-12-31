use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use super::{GroupData, GroupTree, Instance};
use crate::error::{Error, Result};

const MAX_BACKUP_GENERATIONS: usize = 3;

/// Storage data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageData {
    pub instances: Vec<Instance>,
    pub groups: Vec<GroupData>,
    pub updated_at: DateTime<Utc>,
}

/// Session storage handler
pub struct Storage {
    path: PathBuf,
    profile: String,
    lock: Mutex<()>,
}

impl Storage {
    /// Create new storage for a profile
    pub async fn new(profile: &str) -> Result<Self> {
        let base_dir = Self::get_agent_deck_dir()?;
        let profile_dir = base_dir.join("profiles").join(profile);
        fs::create_dir_all(&profile_dir).await?;

        let path = profile_dir.join("sessions.json");

        Ok(Self {
            path,
            profile: profile.to_string(),
            lock: Mutex::new(()),
        })
    }

    /// Get agent-deck base directory
    pub fn get_agent_deck_dir() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
        Ok(home.join(".agent-deck-rs"))
    }

    /// Get profile name
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Load sessions and groups
    pub async fn load(&self) -> Result<(Vec<Instance>, GroupTree)> {
        let _lock = self.lock.lock();

        if !self.path.exists() {
            return Ok((Vec::new(), GroupTree::new()));
        }

        let content = fs::read_to_string(&self.path).await?;
        let data: StorageData = serde_json::from_str(&content)?;

        let tree = GroupTree::from_groups(data.groups);
        Ok((data.instances, tree))
    }

    /// Save sessions and groups
    pub async fn save(&self, instances: &[Instance], tree: &GroupTree) -> Result<()> {
        let _lock = self.lock.lock();

        // Create rolling backups
        self.create_backup().await?;

        // Serialize data
        let data = StorageData {
            instances: instances.to_vec(),
            groups: tree.all_groups(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string_pretty(&data)?;

        // Atomic write: write to temp file, then rename
        let temp_path = self.path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(json.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);

        fs::rename(&temp_path, &self.path).await?;

        Ok(())
    }

    /// Create rolling backup
    async fn create_backup(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        // Roll backups: .bak.2 -> .bak.3, .bak.1 -> .bak.2, .bak -> .bak.1
        for i in (1..MAX_BACKUP_GENERATIONS).rev() {
            let from = if i == 1 {
                self.path.with_extension("bak")
            } else {
                self.path.with_extension(format!("bak.{}", i))
            };
            let to = self.path.with_extension(format!("bak.{}", i + 1));

            if from.exists() {
                // Remove target if exists (fs::rename doesn't overwrite on all platforms)
                if to.exists() {
                    let _ = fs::remove_file(&to).await;
                }
                fs::rename(&from, &to).await?;
            }
        }

        // Current file -> .bak
        let bak = self.path.with_extension("bak");
        if bak.exists() {
            let _ = fs::remove_file(&bak).await;
        }
        fs::copy(&self.path, &bak).await?;

        Ok(())
    }

    /// List all profiles
    pub async fn list_profiles() -> Result<Vec<String>> {
        let base_dir = Self::get_agent_deck_dir()?;
        let profiles_dir = base_dir.join("profiles");

        if !profiles_dir.exists() {
            return Ok(vec!["default".to_string()]);
        }

        let mut entries = fs::read_dir(&profiles_dir).await?;
        let mut profiles = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    profiles.push(name.to_string());
                }
            }
        }

        if profiles.is_empty() {
            profiles.push("default".to_string());
        }

        profiles.sort();
        Ok(profiles)
    }

    /// Create a new profile
    pub async fn create_profile(name: &str) -> Result<()> {
        let base_dir = Self::get_agent_deck_dir()?;
        let profile_dir = base_dir.join("profiles").join(name);

        if profile_dir.exists() {
            return Err(Error::profile(format!("Profile '{}' already exists", name)));
        }

        fs::create_dir_all(&profile_dir).await?;

        // Create empty sessions.json
        let sessions_file = profile_dir.join("sessions.json");
        let data = StorageData {
            instances: Vec::new(),
            groups: Vec::new(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(&sessions_file, json).await?;

        Ok(())
    }

    /// Delete a profile
    pub async fn delete_profile(name: &str) -> Result<()> {
        if name == "default" {
            return Err(Error::profile("Cannot delete default profile"));
        }

        let base_dir = Self::get_agent_deck_dir()?;
        let profile_dir = base_dir.join("profiles").join(name);

        if !profile_dir.exists() {
            return Err(Error::profile(format!("Profile '{}' not found", name)));
        }

        fs::remove_dir_all(&profile_dir).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let profile_dir = dir.path().join("profiles").join("test");
        fs::create_dir_all(&profile_dir).await.unwrap();

        let storage = Storage {
            path: profile_dir.join("sessions.json"),
            profile: "test".to_string(),
            lock: Mutex::new(()),
        };

        let mut instances = Vec::new();
        let instance = Instance::new("test".to_string(), PathBuf::from("/tmp"));
        instances.push(instance);

        let tree = GroupTree::new();

        storage.save(&instances, &tree).await.unwrap();

        let (loaded_instances, _) = storage.load().await.unwrap();
        assert_eq!(loaded_instances.len(), 1);
        assert_eq!(loaded_instances[0].title, "test");
    }
}
