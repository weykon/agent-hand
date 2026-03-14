//! CESP (Coding Event Sound Pack) manifest reader.
//! Compatible with peon-ping's openpeon.json format.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// CESP manifest (openpeon.json / manifest.json)
#[derive(Debug, Deserialize)]
pub struct PackManifest {
    #[allow(dead_code)]
    pub name: Option<String>,
    #[serde(default)]
    pub categories: HashMap<String, CategoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryEntry {
    #[serde(default)]
    pub sounds: Vec<SoundEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SoundEntry {
    pub file: String,
}

/// A loaded sound pack ready to serve sound file paths.
pub struct SoundPack {
    root: PathBuf,
    manifest: PackManifest,
}

impl SoundPack {
    /// Try to load a pack by name from known pack directories.
    /// Search order: ~/.openpeon/packs/<name>, ~/.agent-hand/packs/<name>
    pub fn load(pack_name: &str) -> Option<Self> {
        let home = dirs::home_dir()?;
        let candidates = [
            home.join(".openpeon/packs").join(pack_name),
            home.join(".agent-hand/packs").join(pack_name),
        ];

        for dir in &candidates {
            if let Some(pack) = Self::load_from_dir(dir) {
                return Some(pack);
            }
        }
        None
    }

    /// Discover all installed sound packs from known directories.
    /// Returns a sorted list of pack names.
    pub fn list_installed() -> Vec<String> {
        let mut names = std::collections::BTreeSet::new();
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };

        let dirs = [
            home.join(".openpeon/packs"),
            home.join(".agent-hand/packs"),
        ];

        for parent in &dirs {
            if !parent.is_dir() {
                continue;
            }
            if let Ok(rd) = std::fs::read_dir(parent) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    // Check if this directory has a manifest
                    let has_manifest = path.join("openpeon.json").exists()
                        || path.join("manifest.json").exists();
                    if has_manifest {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            names.insert(name.to_string());
                        }
                    }
                }
            }
        }

        names.into_iter().collect()
    }

    fn load_from_dir(dir: &Path) -> Option<Self> {
        if !dir.is_dir() {
            return None;
        }

        // Try openpeon.json first, then manifest.json
        let manifest_path = if dir.join("openpeon.json").exists() {
            dir.join("openpeon.json")
        } else if dir.join("manifest.json").exists() {
            dir.join("manifest.json")
        } else {
            return None;
        };

        let content = std::fs::read_to_string(&manifest_path).ok()?;
        let manifest: PackManifest = serde_json::from_str(&content).ok()?;

        Some(Self {
            root: dir.to_path_buf(),
            manifest,
        })
    }

    /// Pick a random sound file path for the given CESP category.
    /// Returns None if category doesn't exist or has no sounds.
    pub fn pick_sound(&self, category: &str) -> Option<PathBuf> {
        let entry = self.manifest.categories.get(category)?;
        if entry.sounds.is_empty() {
            return None;
        }

        // Pseudo-random selection using system time nanos
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;
        let idx = nanos % entry.sounds.len();
        let sound = &entry.sounds[idx];

        // Resolve path: if contains '/' → relative to pack root, else → relative to sounds/
        let file_path = if sound.file.contains('/') {
            self.root.join(&sound.file)
        } else {
            self.root.join("sounds").join(&sound.file)
        };

        // Security: ensure resolved path is within pack root
        if let Ok(canonical) = file_path.canonicalize() {
            if let Ok(root_canonical) = self.root.canonicalize() {
                if canonical.starts_with(&root_canonical) && canonical.exists() {
                    return Some(canonical);
                }
            }
        }

        // Fallback: trust the path if it exists (for symlinks etc.)
        if file_path.exists() {
            Some(file_path)
        } else {
            None
        }
    }
}
