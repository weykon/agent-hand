use std::path::PathBuf;

use tokio::fs;
use agent_hooks::ToolAdapter;

use crate::error::{Error, Result};

const HOOK_REL_PATH: &str = ".agent-hand/hooks/log_user_prompt.sh";
const EVENT_BRIDGE_REL_PATH: &str = ".agent-hand/hooks/hook_event_bridge.sh";

/// Ensure the legacy prompt logging hook is installed.
///
/// This uses the original direct approach (not via tool-adapters) because
/// it has special logic for the prompt logging script specifically.
pub async fn ensure_user_prompt_hook() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let hook_path = home.join(HOOK_REL_PATH);

    install_hook_script(
        &hook_path,
        include_str!("../../scripts/claude/log_user_prompt.sh"),
    )
    .await?;

    // Register via tool-adapters ClaudeAdapter for the UserPromptSubmit event
    let adapter = agent_hooks::ClaudeAdapter::new();
    adapter
        .register_hooks(&hook_path)
        .map_err(|e| Error::config(format!("Claude hook registration: {e}")))?;
    Ok(())
}

/// Install the event bridge hook for all supported events across all detected tools.
///
/// Delegates to `agent_hooks` for multi-tool support while maintaining
/// the bridge script installation logic.
pub async fn ensure_event_bridge_hooks() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let bridge_path = home.join(EVENT_BRIDGE_REL_PATH);

    // Install the bridge script
    install_hook_script(
        &bridge_path,
        include_str!("../../scripts/claude/hook_event_bridge.sh"),
    )
    .await?;

    // Ensure events directory exists
    let events_dir = home.join(".agent-hand/events");
    fs::create_dir_all(&events_dir).await?;

    // Register hooks for all detected tools via tool-adapters
    let registered = agent_hooks::auto_register_all(&bridge_path);
    if !registered.is_empty() {
        tracing::info!("Auto-registered hooks for: {}", registered.join(", "));
    }

    Ok(())
}

/// Get the path to the bridge script.
pub fn bridge_script_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(EVENT_BRIDGE_REL_PATH))
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
