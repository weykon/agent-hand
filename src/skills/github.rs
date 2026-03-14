use std::path::Path;

use tokio::process::Command as TokioCommand;

use crate::Result;

/// Status of the `gh` CLI tool.
#[derive(Debug, PartialEq, Eq)]
pub enum GhStatus {
    Ready,
    NotInstalled,
    NotAuthenticated,
}

/// Check whether the GitHub CLI is installed and authenticated.
pub async fn check_gh_prerequisites() -> GhStatus {
    // Check if gh is installed
    let installed = TokioCommand::new("gh")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !installed {
        return GhStatus::NotInstalled;
    }

    // Check if gh is authenticated
    let authed = TokioCommand::new("gh")
        .args(["auth", "status"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !authed {
        return GhStatus::NotAuthenticated;
    }

    GhStatus::Ready
}

/// Create a new GitHub repository for skills and clone it locally.
/// Returns the repository URL.
pub async fn init_skills_repo(name: &str) -> Result<String> {
    let status = check_gh_prerequisites().await;
    match status {
        GhStatus::NotInstalled => {
            return Err(crate::Error::CommandFailed(
                "GitHub CLI (gh) is not installed. Install it from https://cli.github.com"
                    .to_string(),
            ));
        }
        GhStatus::NotAuthenticated => {
            return Err(crate::Error::CommandFailed(
                "GitHub CLI is not authenticated. Run `gh auth login` first.".to_string(),
            ));
        }
        GhStatus::Ready => {}
    }

    let repo_path = super::SkillsRegistry::default_repo_path()?;
    if let Some(parent) = repo_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create the remote repo (private by default)
    let output = TokioCommand::new("gh")
        .args(["repo", "create", name, "--private", "--clone"])
        .current_dir(repo_path.parent().unwrap_or(Path::new(".")))
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to create repo: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If the repo already exists, try to clone it instead
        if stderr.contains("already exists") {
            return clone_existing_repo(name, &repo_path).await;
        }
        return Err(crate::Error::CommandFailed(format!(
            "Failed to create repo: {stderr}"
        )));
    }

    // Get the repo URL
    let url_output = TokioCommand::new("gh")
        .args(["repo", "view", name, "--json", "url", "-q", ".url"])
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to get repo URL: {e}")))?;

    let url = String::from_utf8_lossy(&url_output.stdout).trim().to_string();

    // Rename the cloned directory to "repo" if gh created it with the repo name
    let created_dir = repo_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(name);
    if created_dir.exists() && !repo_path.exists() {
        std::fs::rename(&created_dir, &repo_path)?;
    }

    Ok(url)
}

/// Clone an existing repository by name.
async fn clone_existing_repo(name: &str, repo_path: &Path) -> Result<String> {
    if repo_path.exists() {
        // Already cloned, just get the URL
        let url_output = TokioCommand::new("gh")
            .args(["repo", "view", name, "--json", "url", "-q", ".url"])
            .output()
            .await
            .map_err(|e| crate::Error::CommandFailed(format!("Failed to get repo URL: {e}")))?;
        return Ok(String::from_utf8_lossy(&url_output.stdout)
            .trim()
            .to_string());
    }

    let output = TokioCommand::new("gh")
        .args(["repo", "clone", name, &repo_path.to_string_lossy()])
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to clone repo: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "Failed to clone repo: {stderr}"
        )));
    }

    let url_output = TokioCommand::new("gh")
        .args(["repo", "view", name, "--json", "url", "-q", ".url"])
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to get repo URL: {e}")))?;

    Ok(String::from_utf8_lossy(&url_output.stdout)
        .trim()
        .to_string())
}

/// Pull the latest changes from the remote repository.
pub async fn sync_repo(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(crate::Error::CommandFailed(
            "Skills repo directory does not exist. Run `agent-hand skills init` first.".to_string(),
        ));
    }

    let output = TokioCommand::new("git")
        .args(["pull", "--rebase"])
        .current_dir(path)
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to sync repo: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "Failed to sync repo: {stderr}"
        )));
    }

    Ok(())
}

/// Stage all changes, commit, and push to remote.
pub async fn push_repo(path: &Path, message: &str) -> Result<()> {
    if !path.exists() {
        return Err(crate::Error::CommandFailed(
            "Skills repo directory does not exist.".to_string(),
        ));
    }

    // Stage all changes
    let add = TokioCommand::new("git")
        .args(["add", "-A"])
        .current_dir(path)
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("git add failed: {e}")))?;

    if !add.status.success() {
        let stderr = String::from_utf8_lossy(&add.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "git add failed: {stderr}"
        )));
    }

    // Check if there are staged changes
    let diff = TokioCommand::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(path)
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("git diff failed: {e}")))?;

    if diff.status.success() {
        // No changes to commit
        return Ok(());
    }

    // Commit
    let commit = TokioCommand::new("git")
        .args(["commit", "-m", message])
        .current_dir(path)
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("git commit failed: {e}")))?;

    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "git commit failed: {stderr}"
        )));
    }

    // Push
    let push = TokioCommand::new("git")
        .args(["push"])
        .current_dir(path)
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("git push failed: {e}")))?;

    if !push.status.success() {
        let stderr = String::from_utf8_lossy(&push.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "git push failed: {stderr}"
        )));
    }

    Ok(())
}

/// Add a community skill by cloning from a GitHub URL into the target directory.
pub async fn add_community_skill(url: &str, target: &Path) -> Result<()> {
    if target.exists() {
        return Err(crate::Error::InvalidInput(format!(
            "Target directory already exists: {}",
            target.display()
        )));
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = TokioCommand::new("git")
        .args(["clone", "--depth", "1", url, &target.to_string_lossy()])
        .output()
        .await
        .map_err(|e| crate::Error::CommandFailed(format!("Failed to clone skill: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::CommandFailed(format!(
            "Failed to clone skill from {url}: {stderr}"
        )));
    }

    Ok(())
}
