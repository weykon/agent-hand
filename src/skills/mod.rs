pub mod github;
pub mod linker;
pub mod manager_skill;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::session::Storage;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsRegistry {
    pub repo_url: Option<String>,
    pub repo_path: Option<PathBuf>,
    pub skills: Vec<SkillEntry>,
    pub last_synced: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub path: String,
    pub linked_to: Vec<SkillLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLink {
    pub project_path: PathBuf,
    pub cli: SkillCli,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillCli {
    Claude,
    Codex,
    Cursor,
}

impl std::fmt::Display for SkillCli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillCli::Claude => write!(f, "claude"),
            SkillCli::Codex => write!(f, "codex"),
            SkillCli::Cursor => write!(f, "cursor"),
        }
    }
}

/// A built-in skill that ships embedded in the binary.
pub struct BuiltInSkill {
    pub name: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

/// Built-in skills with their SKILL.md content embedded at compile time.
pub const BUILT_IN_SKILLS: &[BuiltInSkill] = &[
    BuiltInSkill {
        name: "bridge-overview",
        description: "Overview of the agent-hand-bridge binary and when to use each mode",
        content: include_str!("../../skills/bridge-overview/SKILL.md"),
    },
    BuiltInSkill {
        name: "canvas-ops",
        description: "Operate the agent-hand canvas workflow editor via agent-hand-bridge",
        content: include_str!("../../skills/canvas-ops/SKILL.md"),
    },
    BuiltInSkill {
        name: "session-manager",
        description: "Manage agent-hand tmux sessions via the CLI",
        content: include_str!("../../skills/session-manager/SKILL.md"),
    },
    BuiltInSkill {
        name: "workspace-ops",
        description: "Manage agent-hand workspace — sessions, groups, canvas, progress",
        content: include_str!("../../skills/workspace-ops/SKILL.md"),
    },
    BuiltInSkill {
        name: "canvas-render",
        description: "Render agent-driven canvas visualizations from runtime coordination artifacts",
        content: include_str!("../../skills/canvas-render/SKILL.md"),
    },
];

/// Check if a skill name is a built-in skill.
pub fn is_built_in(name: &str) -> bool {
    BUILT_IN_SKILLS.iter().any(|s| s.name == name)
}

/// Returns ~/.agent-hand/skills/builtin/
pub fn builtin_dir() -> Result<std::path::PathBuf> {
    let dir = Storage::get_agent_hand_dir()?.join("skills").join("builtin");
    Ok(dir)
}

/// Write all built-in SKILL.md files to ~/.agent-hand/skills/builtin/<name>/SKILL.md.
/// Overwrites on every call so upgrades propagate automatically.
pub fn seed_builtins() -> Result<std::path::PathBuf> {
    let base = builtin_dir()?;
    for skill in BUILT_IN_SKILLS {
        let dir = base.join(skill.name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("SKILL.md"), skill.content)?;
    }
    Ok(base)
}

impl SkillEntry {
    /// Read a short content preview from the SKILL.md body (after frontmatter).
    pub fn content_preview(&self, repo_path: &std::path::Path) -> Option<String> {
        // Try repo path first, then fall back to builtin dir
        let repo_skill_md = repo_path.join(&self.path).join("SKILL.md");
        let content = std::fs::read_to_string(&repo_skill_md)
            .or_else(|_| {
                let builtin = builtin_dir()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;
                std::fs::read_to_string(builtin.join(&self.path).join("SKILL.md"))
            })
            .ok()?;
        Self::extract_preview_body(&content)
    }

    /// Extract the body preview (up to 5 lines after frontmatter) from SKILL.md content.
    fn extract_preview_body(content: &str) -> Option<String> {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return Some(trimmed.lines().take(5).collect::<Vec<_>>().join("\n"));
        }
        let after_first = &trimmed[3..];
        let end_idx = after_first.find("---")?;
        let body = after_first[end_idx + 3..].trim();
        if body.is_empty() {
            return None;
        }
        Some(body.lines().take(5).collect::<Vec<_>>().join("\n"))
    }
}

impl SkillsRegistry {
    /// Get all projects a skill is linked to.
    pub fn linked_projects(&self, name: &str) -> Vec<PathBuf> {
        self.find_skill(name)
            .map(|s| s.linked_to.iter().map(|l| l.project_path.clone()).collect())
            .unwrap_or_default()
    }

    /// Path to the registry JSON file (~/.agent-hand/skills/registry.json).
    fn registry_path() -> Result<PathBuf> {
        let dir = Storage::get_agent_hand_dir()?.join("skills");
        Ok(dir.join("registry.json"))
    }

    /// Load the registry from disk. Returns a default empty registry if the file
    /// does not exist yet.
    pub fn load() -> Result<Self> {
        let path = Self::registry_path()?;
        if !path.exists() {
            return Ok(Self {
                repo_url: None,
                repo_path: None,
                skills: Vec::new(),
                last_synced: None,
            });
        }
        let data = std::fs::read_to_string(&path)?;
        let registry: Self = serde_json::from_str(&data)?;
        Ok(registry)
    }

    /// Save the registry to disk, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::registry_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Default repo path (~/.agent-hand/skills/repo/).
    pub fn default_repo_path() -> Result<PathBuf> {
        let dir = Storage::get_agent_hand_dir()?.join("skills").join("repo");
        Ok(dir)
    }

    /// Find a skill by name.
    pub fn find_skill(&self, name: &str) -> Option<&SkillEntry> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Find a skill by name (mutable).
    pub fn find_skill_mut(&mut self, name: &str) -> Option<&mut SkillEntry> {
        self.skills.iter_mut().find(|s| s.name == name)
    }

    /// Ensure all built-in skills exist in the registry.
    /// Uses the skill name as the path so content_preview() resolves them via builtin_dir().
    pub fn ensure_builtins_in_registry(&mut self) {
        for skill in BUILT_IN_SKILLS {
            if self.find_skill(skill.name).is_none() {
                self.skills.push(SkillEntry {
                    name: skill.name.to_string(),
                    description: skill.description.to_string(),
                    path: skill.name.to_string(),
                    linked_to: Vec::new(),
                });
            }
        }
    }

    /// Scan the repo directory for SKILL.md files and update the skills list.
    /// New skills are added; existing skills retain their links.
    pub fn scan_repo(&mut self) -> Result<()> {
        let repo_path = match &self.repo_path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        if !repo_path.exists() {
            return Ok(());
        }

        let mut discovered: Vec<(String, String, String)> = Vec::new();
        scan_skills_recursive(&repo_path, &repo_path, &mut discovered)?;

        for (name, description, rel_path) in discovered {
            if self.find_skill(&name).is_none() {
                self.skills.push(SkillEntry {
                    name,
                    description,
                    path: rel_path,
                    linked_to: Vec::new(),
                });
            }
        }

        Ok(())
    }
}

/// Recursively scan for SKILL.md files under `base`, collecting (name, description, relative_path).
fn scan_skills_recursive(
    base: &std::path::Path,
    current: &std::path::Path,
    results: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_skills_recursive(base, &path, results)?;
        } else if path.file_name().map(|f| f == "SKILL.md").unwrap_or(false) {
            let content = std::fs::read_to_string(&path)?;
            if let Some((name, description)) = parse_skill_frontmatter(&content) {
                let rel = path
                    .parent()
                    .unwrap_or(&path)
                    .strip_prefix(base)
                    .unwrap_or(std::path::Path::new(""))
                    .to_string_lossy()
                    .to_string();
                results.push((name, description, rel));
            }
        }
    }

    Ok(())
}

/// Parse YAML frontmatter from a SKILL.md file, extracting `name` and `description`.
///
/// Expects the format:
/// ```text
/// ---
/// name: my-skill
/// description: A brief description
/// ---
/// ```
pub fn parse_skill_frontmatter(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let end_idx = after_first.find("---")?;
    let frontmatter = &after_first[..end_idx];

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }

    match (name, description) {
        (Some(n), Some(d)) if !n.is_empty() => Some((n, d)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = r#"---
name: code-review
description: Automated code review skill
---
# Code Review Skill
"#;
        let result = parse_skill_frontmatter(content);
        assert_eq!(
            result,
            Some(("code-review".to_string(), "Automated code review skill".to_string()))
        );
    }

    #[test]
    fn test_parse_frontmatter_quoted() {
        let content = r#"---
name: "my-skill"
description: 'A skill with quotes'
---
"#;
        let result = parse_skill_frontmatter(content);
        assert_eq!(
            result,
            Some(("my-skill".to_string(), "A skill with quotes".to_string()))
        );
    }

    #[test]
    fn test_parse_frontmatter_missing_delimiter() {
        let content = "name: broken\ndescription: no frontmatter";
        assert_eq!(parse_skill_frontmatter(content), None);
    }

    #[test]
    fn test_parse_frontmatter_missing_fields() {
        let content = "---\nname: only-name\n---\n";
        assert_eq!(parse_skill_frontmatter(content), None);
    }
}
