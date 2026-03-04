use std::path::PathBuf;

use serde_json::{json, Value};
use tokio::fs;

use crate::error::{Error, Result};

const SETTINGS_REL_PATH: &str = ".claude/settings.json";
const HOOK_REL_PATH: &str = ".agent-hand/hooks/log_user_prompt.sh";
const EVENT_BRIDGE_REL_PATH: &str = ".agent-hand/hooks/hook_event_bridge.sh";

/// Claude Code hook event types we register for status detection.
const EVENT_HOOK_TYPES: &[&str] = &[
    "Stop",
    "Notification",
    "UserPromptSubmit",
    "SubagentStart",
    "PreCompact",
];

/// Ensure the legacy prompt logging hook is installed.
pub async fn ensure_user_prompt_hook() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let settings_path = home.join(SETTINGS_REL_PATH);
    let hook_path = home.join(HOOK_REL_PATH);

    install_hook_script(
        &hook_path,
        include_str!("../../scripts/claude/log_user_prompt.sh"),
    )
    .await?;
    ensure_settings_hook_for_event(&settings_path, &hook_path, "UserPromptSubmit").await
}

/// Install the event bridge hook for all supported Claude Code events.
/// This enables event-driven status detection (replaces polling).
pub async fn ensure_event_bridge_hooks() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let settings_path = home.join(SETTINGS_REL_PATH);
    let bridge_path = home.join(EVENT_BRIDGE_REL_PATH);

    install_hook_script(
        &bridge_path,
        include_str!("../../scripts/claude/hook_event_bridge.sh"),
    )
    .await?;

    // Ensure events directory exists
    let events_dir = home.join(".agent-hand/events");
    fs::create_dir_all(&events_dir).await?;

    // Register the bridge script for each event type
    for event_type in EVENT_HOOK_TYPES {
        ensure_settings_hook_for_event(&settings_path, &bridge_path, event_type).await?;
    }

    Ok(())
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

/// Register a hook command for a specific event type in ~/.claude/settings.json.
async fn ensure_settings_hook_for_event(
    settings_path: &PathBuf,
    hook_path: &PathBuf,
    event_type: &str,
) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut root: Value = match fs::read_to_string(settings_path).await {
        Ok(content) => serde_json::from_str(&content)?,
        Err(_) => json!({}),
    };

    let Some(obj) = root.as_object_mut() else {
        return Err(Error::config(
            "Invalid root JSON in ~/.claude/settings.json",
        ));
    };

    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| Error::config("Invalid hooks format in ~/.claude/settings.json"))?;

    let event_hooks = hooks_obj
        .entry(event_type)
        .or_insert_with(|| json!([]));
    if !event_hooks.is_array() {
        *event_hooks = json!([]);
    }
    let event_hooks_arr = event_hooks.as_array_mut().ok_or_else(|| {
        Error::config(format!(
            "Invalid {} hooks format in ~/.claude/settings.json",
            event_type
        ))
    })?;

    let hook_cmd = hook_path.to_string_lossy().to_string();
    let already_set = event_hooks_arr.iter().any(|item| {
        item.get("hooks")
            .and_then(|h| h.as_array())
            .map(|arr| {
                arr.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|c| c == hook_cmd)
                })
            })
            .unwrap_or(false)
    });

    if !already_set {
        event_hooks_arr.push(json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": hook_cmd
            }]
        }));

        // Back up before writing
        if settings_path.exists() {
            let bak_path = settings_path.with_extension("json.bak");
            let _ = std::fs::copy(settings_path, &bak_path);
        }

        let output = serde_json::to_string_pretty(&root)?;
        fs::write(settings_path, output).await?;
    }

    Ok(())
}
