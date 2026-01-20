use std::path::PathBuf;

use serde_json::{json, Value};
use tokio::fs;

use crate::error::{Error, Result};

const SETTINGS_REL_PATH: &str = ".claude/settings.json";
const HOOK_REL_PATH: &str = ".agent-hand/hooks/log_user_prompt.sh";

pub async fn ensure_user_prompt_hook() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| Error::config("Cannot determine home directory"))?;
    let settings_path = home.join(SETTINGS_REL_PATH);
    let hook_path = home.join(HOOK_REL_PATH);

    install_hook_script(&hook_path).await?;
    ensure_settings_hook(&settings_path, &hook_path).await
}

async fn install_hook_script(hook_path: &PathBuf) -> Result<()> {
    let hook_dir = hook_path
        .parent()
        .ok_or_else(|| Error::config("Invalid hook path"))?;
    fs::create_dir_all(hook_dir).await?;

    let script = include_str!("../../scripts/claude/log_user_prompt.sh");
    let write_needed = match fs::read_to_string(hook_path).await {
        Ok(existing) => existing != script,
        Err(_) => true,
    };

    if write_needed {
        fs::write(hook_path, script).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(hook_path, perms)?;
        }
    }

    Ok(())
}

async fn ensure_settings_hook(settings_path: &PathBuf, hook_path: &PathBuf) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut root: Value = match fs::read_to_string(settings_path).await {
        Ok(content) => serde_json::from_str(&content)?,
        Err(_) => json!({}),
    };

    if let Some(obj) = root.as_object_mut() {
        let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
        if !hooks.is_object() {
            *hooks = json!({});
        }
        let hooks_obj = hooks
            .as_object_mut()
            .ok_or_else(|| Error::config("Invalid hooks format in ~/.claude/settings.json"))?;

        let user_hooks = hooks_obj
            .entry("UserPromptSubmit")
            .or_insert_with(|| json!([]));
        if !user_hooks.is_array() {
            *user_hooks = json!([]);
        }
        let user_hooks_arr = user_hooks.as_array_mut().ok_or_else(|| {
            Error::config("Invalid UserPromptSubmit hooks format in ~/.claude/settings.json")
        })?;

        let hook_cmd = hook_path.to_string_lossy().to_string();
        let already_set = user_hooks_arr.iter().any(|item| {
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
            user_hooks_arr.push(json!({
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": hook_cmd
                }]
            }));
        }
    } else {
        return Err(Error::config(
            "Invalid root JSON in ~/.claude/settings.json",
        ));
    }

    if settings_path.exists() {
        let bak_path = settings_path.with_extension("json.bak");
        let _ = std::fs::copy(settings_path, &bak_path);
    }

    let output = serde_json::to_string_pretty(&root)?;
    fs::write(settings_path, output).await?;

    Ok(())
}
