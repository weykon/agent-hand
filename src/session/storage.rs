use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use super::{GroupData, GroupTree, Instance};
use crate::error::{Error, Result};

fn copy_dir_recursive_sync(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for ent in std::fs::read_dir(src)? {
        let ent = ent?;
        let file_type = ent.file_type()?;
        let from = ent.path();
        let to = dst.join(ent.file_name());
        if file_type.is_dir() {
            copy_dir_recursive_sync(&from, &to)?;
        } else if file_type.is_file() {
            let _ = std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

async fn copy_dir_recursive(src: PathBuf, dst: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || copy_dir_recursive_sync(&src, &dst))
        .await
        .map_err(|e| Error::Other(format!("Migration task failed: {e}")))??;
    Ok(())
}

async fn sessions_instances_count(path: &PathBuf) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let content = fs::read_to_string(path).await?;
    let v: serde_json::Value = serde_json::from_str(&content)?;
    Ok(v.get("instances")
        .and_then(|x| x.as_array())
        .map(|a| a.len())
        .unwrap_or(0))
}

async fn profiles_have_instances(profiles_dir: &PathBuf) -> Result<bool> {
    if !profiles_dir.exists() {
        return Ok(false);
    }
    let mut rd = fs::read_dir(profiles_dir).await?;
    while let Some(ent) = rd.next_entry().await? {
        if !ent.file_type().await?.is_dir() {
            continue;
        }
        let sessions = ent.path().join("sessions.json");
        if sessions_instances_count(&sessions).await? > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

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
        Self::migrate_legacy_profiles_if_needed().await?;

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

    /// Get agent-hand base directory
    ///
    /// Backward-compat:
    /// - If `~/.agent-deck-rs` exists and `~/.agent-hand` does not, use the old dir.
    /// - If both exist, we still use `~/.agent-hand` (and may migrate legacy profiles on startup).
    pub fn get_agent_hand_dir() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;

        let new_dir = home.join(".agent-hand");
        let old_dir = home.join(".agent-deck-rs");

        if !new_dir.exists() && old_dir.exists() {
            Ok(old_dir)
        } else {
            Ok(new_dir)
        }
    }

    async fn migrate_legacy_profiles_if_needed() -> Result<()> {
        let home =
            dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
        let new_dir = home.join(".agent-hand");
        let old_dir = home.join(".agent-deck-rs");

        if !new_dir.exists() || !old_dir.exists() {
            return Ok(());
        }

        let old_profiles = old_dir.join("profiles");
        if !old_profiles.exists() {
            return Ok(());
        }

        let new_profiles = new_dir.join("profiles");
        if !new_profiles.exists() {
            fs::create_dir_all(&new_profiles).await?;
        }

        let old_has_instances = profiles_have_instances(&old_profiles).await?;
        if !old_has_instances {
            return Ok(());
        }

        let new_has_instances = profiles_have_instances(&new_profiles).await?;
        if new_has_instances {
            return Ok(());
        }

        let mut rd = fs::read_dir(&old_profiles).await?;
        while let Some(ent) = rd.next_entry().await? {
            let file_type = ent.file_type().await?;
            if !file_type.is_dir() {
                continue;
            }

            let name = ent.file_name();
            let src_profile = ent.path();
            let dst_profile = new_profiles.join(&name);
            fs::create_dir_all(&dst_profile).await?;

            // Prefer migrating sessions.json even if the directory already exists.
            let src_sessions = src_profile.join("sessions.json");
            let dst_sessions = dst_profile.join("sessions.json");
            let src_count = sessions_instances_count(&src_sessions).await?;
            let dst_count = sessions_instances_count(&dst_sessions).await?;
            if src_count > 0 && dst_count == 0 {
                let _ = fs::copy(&src_sessions, &dst_sessions).await?;
            }

            // Copy the rest (e.g. backups) only if missing.
            let mut src_rd = fs::read_dir(&src_profile).await?;
            while let Some(src_ent) = src_rd.next_entry().await? {
                let src_type = src_ent.file_type().await?;
                let from = src_ent.path();
                let to = dst_profile.join(src_ent.file_name());
                if to.exists() {
                    continue;
                }
                if src_type.is_dir() {
                    copy_dir_recursive(from, to).await?;
                } else if src_type.is_file() {
                    let _ = fs::copy(&from, &to).await?;
                }
            }
        }

        Ok(())
    }

    /// Backward-compatible alias
    pub fn get_agent_deck_dir() -> Result<PathBuf> {
        Self::get_agent_hand_dir()
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
