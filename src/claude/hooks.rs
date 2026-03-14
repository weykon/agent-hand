use std::path::PathBuf;

use tokio::fs;

use crate::error::{Error, Result};

const EVENT_BRIDGE_REL_PATH: &str = ".agent-hand/hooks/hook_event_bridge.sh";
const BRIDGE_BINARY_REL_PATH: &str = ".agent-hand/bin/agent-hand-bridge";

/// Install the event bridge hook for all supported events across all detected tools.
///
/// Prefers the Rust `agent-hand-bridge` binary (zero-dependency, fast).
/// Falls back to the legacy shell+python bridge script if the binary is not found
/// (e.g. during development when only `cargo run` is used).
pub async fn ensure_event_bridge_hooks() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;

    // Ensure events directory exists
    let events_dir = home.join(".agent-hand/events");
    fs::create_dir_all(&events_dir).await?;

    // Try to install the Rust bridge binary; fall back to legacy shell script.
    let hook_cmd_path = if let Some(installed) = install_bridge_binary(&home).await? {
        installed
    } else {
        // Fallback: install the legacy shell script
        let bridge_path = home.join(EVENT_BRIDGE_REL_PATH);
        install_hook_script(
            &bridge_path,
            include_str!("../../scripts/claude/hook_event_bridge.sh"),
        )
        .await?;
        bridge_path
    };

    // Register hooks for all detected tools via tool-adapters
    let registered = agent_hooks::auto_register_all(&hook_cmd_path);
    if !registered.is_empty() {
        tracing::info!("Auto-registered hooks for: {}", registered.join(", "));
    }

    Ok(())
}

/// Get the path to the bridge script.
pub fn bridge_script_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(EVENT_BRIDGE_REL_PATH))
}

/// Try to locate `agent-hand-bridge` next to the running `agent-hand` binary
/// and copy it to `~/.agent-hand/bin/agent-hand-bridge`.
///
/// Returns `Some(installed_path)` on success, `None` if the source binary doesn't exist
/// (e.g. dev environment using `cargo run`).
async fn install_bridge_binary(home: &PathBuf) -> Result<Option<PathBuf>> {
    // Find the source binary — it should sit alongside the running executable.
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let exe_dir = match current_exe.parent() {
        Some(d) => d,
        None => return Ok(None),
    };

    let source = exe_dir.join("agent-hand-bridge");
    if !source.exists() {
        tracing::debug!(
            "Bridge binary not found at {}; falling back to shell script",
            source.display()
        );
        return Ok(None);
    }

    let dest = home.join(BRIDGE_BINARY_REL_PATH);
    let dest_dir = dest
        .parent()
        .ok_or_else(|| Error::config("Invalid bridge binary path"))?;
    fs::create_dir_all(dest_dir).await?;

    // Only copy if source is newer than dest (or dest doesn't exist).
    let needs_copy = match (fs::metadata(&source).await, fs::metadata(&dest).await) {
        (Ok(src_meta), Ok(dst_meta)) => {
            let src_modified = src_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let dst_modified = dst_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            src_modified > dst_modified
        }
        (Ok(_), Err(_)) => true, // dest doesn't exist
        _ => return Ok(None),     // source inaccessible
    };

    if needs_copy {
        fs::copy(&source, &dest).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }
        tracing::info!(
            "Installed bridge binary: {} → {}",
            source.display(),
            dest.display()
        );
    }

    Ok(Some(dest))
}

async fn install_hook_script(hook_path: &PathBuf, script_content: &str) -> Result<()> {
    let hook_dir = hook_path
        .parent()
        .ok_or_else(|| Error::config("Invalid hook path"))?;
    fs::create_dir_all(hook_dir).await?;

    let write_needed = match fs::read_to_string(hook_path).await {
        Ok(existing) => existing != script_content,
        Err(_) => true,
    };

    if write_needed {
        fs::write(hook_path, script_content).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(hook_path, perms)?;
        }
    }

    Ok(())
}
