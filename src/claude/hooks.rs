use std::path::PathBuf;

use serde_json::{json, Value};
use tokio::fs;

use crate::error::{Error, Result};

const HOOK_REL_PATH: &str = ".agent-hand/hooks/log_user_prompt.sh";
const EVENT_BRIDGE_REL_PATH: &str = ".agent-hand/hooks/hook_event_bridge.sh";

/// Events where log_user_prompt.sh was incorrectly registered by a previous bug.
/// It should ONLY be on UserPromptSubmit.
const MISREGISTERED_EVENTS: &[&str] = &["Stop", "Notification", "SubagentStart", "PreCompact"];

/// Ensure the prompt logging hook is installed for UserPromptSubmit only.
///
/// Directly writes to `~/.claude/settings.json` instead of using
/// `adapter.register_hooks()` which would register to ALL events.
/// Also cleans up any misregistered entries from other events.
pub async fn ensure_user_prompt_hook() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let hook_path = home.join(HOOK_REL_PATH);

    install_hook_script(
        &hook_path,
        include_str!("../../scripts/claude/log_user_prompt.sh"),
    )
    .await?;

    let hook_cmd = hook_path.to_string_lossy().to_string();
    let settings_path = home.join(".claude/settings.json");

    // Register only for UserPromptSubmit
    ensure_settings_hook_for_event(&settings_path, &hook_cmd, "UserPromptSubmit")?;

    // Clean up misregistered entries from other events
    // (previous bug: adapter.register_hooks registered to all 5 events)
    for event in MISREGISTERED_EVENTS {
        remove_settings_hook_for_event(&settings_path, &hook_cmd, event)?;
    }

    Ok(())
}

/// Register a hook command for a single event in Claude's settings.json.
fn ensure_settings_hook_for_event(
    settings_path: &std::path::Path,
    hook_cmd: &str,
    event: &str,
) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)?;
        serde_json::from_str(&content)?
    } else {
        json!({})
    };

    let obj = root
        .as_object_mut()
        .ok_or_else(|| Error::config("Invalid settings.json root"))?;

    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| Error::config("Invalid hooks format"))?;

    let event_hooks = hooks_obj.entry(event).or_insert_with(|| json!([]));
    if !event_hooks.is_array() {
        *event_hooks = json!([]);
    }
    let arr = event_hooks
        .as_array_mut()
        .ok_or_else(|| Error::config("Invalid event hooks format"))?;

    let already = arr.iter().any(|item| {
        item.get("hooks")
            .and_then(|h| h.as_array())
            .map(|a| {
                a.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|c| c == hook_cmd)
                })
            })
            .unwrap_or(false)
    });

    if !already {
        arr.push(json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": hook_cmd
            }]
        }));
        let output = serde_json::to_string_pretty(&root)?;
        std::fs::write(settings_path, output)?;
    }

    Ok(())
}

/// Remove a hook command from a specific event in Claude's settings.json.
fn remove_settings_hook_for_event(
    settings_path: &std::path::Path,
    hook_cmd: &str,
    event: &str,
) -> Result<()> {
    if !settings_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(settings_path)?;
    let mut root: Value = serde_json::from_str(&content)?;

    let modified = if let Some(hooks) = root
        .as_object_mut()
        .and_then(|o| o.get_mut("hooks"))
        .and_then(|h| h.as_object_mut())
    {
        if let Some(arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) {
            let before = arr.len();
            arr.retain(|item| {
                !item
                    .get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|a| {
                        a.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .is_some_and(|c| c == hook_cmd)
                        })
                    })
                    .unwrap_or(false)
            });
            arr.len() != before
        } else {
            false
        }
    } else {
        false
    };

    if modified {
        let output = serde_json::to_string_pretty(&root)?;
        std::fs::write(settings_path, output)?;
    }

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
