use std::path::{Path, PathBuf};

use super::SkillCli;
use crate::Result;

/// Get the target directory for a skill symlink given the project path, CLI type,
/// and skill name.
fn target_dir(project_path: &Path, cli: SkillCli, skill_name: &str) -> PathBuf {
    match cli {
        SkillCli::Claude => project_path
            .join(".claude")
            .join("skills")
            .join(skill_name),
        SkillCli::Codex => project_path
            .join(".agents")
            .join("skills")
            .join(skill_name),
        SkillCli::Cursor => project_path
            .join(".cursor")
            .join("skills")
            .join(skill_name),
    }
}

/// Link a skill source directory into a project for the given CLI.
///
/// On Unix, creates a symlink. On Windows, performs a directory copy.
pub fn link_skill(
    skill_source: &Path,
    project_path: &Path,
    cli: SkillCli,
    skill_name: &str,
) -> Result<()> {
    if !skill_source.exists() {
        return Err(crate::Error::InvalidInput(format!(
            "Skill source does not exist: {}",
            skill_source.display()
        )));
    }

    let target = target_dir(project_path, cli, skill_name);

    // Remove existing link/directory if present
    if target.exists() || target.symlink_metadata().is_ok() {
        remove_link_or_dir(&target)?;
    }

    // Create parent directories
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    create_link(skill_source, &target)?;

    Ok(())
}

/// Remove a skill link from a project for the given CLI.
pub fn unlink_skill(project_path: &Path, cli: SkillCli, skill_name: &str) -> Result<()> {
    let target = target_dir(project_path, cli, skill_name);

    if !target.exists() && target.symlink_metadata().is_err() {
        return Err(crate::Error::InvalidInput(format!(
            "Skill '{}' is not linked for {} in {}",
            skill_name,
            cli,
            project_path.display()
        )));
    }

    remove_link_or_dir(&target)?;

    Ok(())
}

/// Detect which AI CLI tools are configured in the given project directory.
pub fn detect_cli(project_path: &Path) -> Vec<SkillCli> {
    let mut detected = Vec::new();

    if project_path.join(".claude").exists() {
        detected.push(SkillCli::Claude);
    }
    if project_path.join(".agents").exists() {
        detected.push(SkillCli::Codex);
    }
    if project_path.join(".cursor").exists() {
        detected.push(SkillCli::Cursor);
    }

    // Default to Claude if nothing is detected
    if detected.is_empty() {
        detected.push(SkillCli::Claude);
    }

    detected
}

/// Create a symlink (Unix) or copy directory (Windows).
#[cfg(unix)]
fn create_link(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(windows)]
fn create_link(source: &Path, target: &Path) -> Result<()> {
    copy_dir_recursive(source, target)?;
    Ok(())
}

/// Remove a symlink or directory.
fn remove_link_or_dir(path: &Path) -> Result<()> {
    let meta = match path.symlink_metadata() {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        {
            std::fs::remove_file(path)?;
        }
        #[cfg(windows)]
        {
            // On Windows, symlinks to dirs need remove_dir
            if meta.is_dir() {
                std::fs::remove_dir(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
        }
    } else if meta.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }

    Ok(())
}

/// Recursively copy a directory (used on Windows as symlink fallback).
#[cfg(windows)]
fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = target.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
