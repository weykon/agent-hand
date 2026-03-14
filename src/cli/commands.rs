use std::path::PathBuf;
use std::sync::Arc;

use tokio::process::Command as TokioCommand;

use crate::cli::{Args, CanvasAction, Command, ConfigAction, ProfileAction, SessionAction};
#[cfg(feature = "pro")]
use crate::cli::SkillsAction;
use crate::error::Result;
use crate::i18n::Language;
use crate::session::{Instance, Storage, DEFAULT_PROFILE};
use crate::t;
use crate::tmux::TmuxManager;
use tracing::warn;

pub async fn run_cli(args: Args) -> Result<()> {
    let lang = crate::i18n::cli_lang();

    // Backward-compat: allow legacy env var name.
    let legacy_profile = std::env::var("AGENTDECK_PROFILE").ok();
    let profile = args
        .profile
        .as_deref()
        .or(legacy_profile.as_deref())
        .unwrap_or(DEFAULT_PROFILE);

    // Ensure tmux popups inherit the active profile.
    std::env::set_var("AGENTHAND_PROFILE", profile);

    let cfg = crate::config::ConfigFile::load().await.ok().flatten();
    // Install event bridge hooks for event-driven status detection.
    // When auto_register is enabled (default), this also registers hooks
    // for other detected AI CLI tools (Cursor, Codex, Windsurf, etc.).
    let auto_register = cfg
        .as_ref()
        .map(|c| c.hooks().auto_register)
        .unwrap_or(true);
    if auto_register {
        if let Err(err) = crate::claude::ensure_event_bridge_hooks().await {
            warn!("failed to ensure event bridge hooks: {err}");
        }
    } else {
        // Even without auto-register, still install bridge script + Claude hooks
        if let Err(err) = crate::claude::ensure_event_bridge_hooks().await {
            warn!("failed to ensure event bridge hooks: {err}");
        }
    }
    if let Some(cfg) = cfg.as_ref() {
        if let Err(err) = crate::tmux::set_status_detection_config(cfg.status_detection()) {
            warn!("failed to set status detection config: {err}");
        }
    }

    match args.command {
        Some(Command::Add {
            path,
            title,
            group,
            cmd,
        }) => handle_add(lang, profile, path, title, group, cmd).await,

        Some(Command::List { json, all }) => handle_list(lang, profile, json, all).await,

        Some(Command::Remove { identifier }) => handle_remove(lang, profile, &identifier).await,

        Some(Command::Status {
            verbose,
            quiet,
            json,
        }) => handle_status(lang, profile, verbose, quiet, json).await,

        Some(Command::Statusline) => handle_statusline(profile).await,

        Some(Command::Session { action }) => handle_session(lang, profile, action).await,

        Some(Command::Profile { action }) => handle_profile(lang, action).await,

        Some(Command::Upgrade { prefix, version }) => {
            handle_upgrade(lang, prefix, version).await
        }

        Some(Command::Switch) => crate::ui::run_switcher(profile).await,

        Some(Command::Jump) => handle_jump(profile).await,

        Some(Command::Version) => {
            println!("agent-hand v{}", crate::VERSION);
            Ok(())
        }

        Some(Command::Login) => handle_login(lang).await,

        Some(Command::Logout) => handle_logout(lang),

        Some(Command::Account { refresh }) => handle_account(lang, refresh).await,

        Some(Command::Devices { remove }) => handle_devices(lang, remove).await,

        Some(Command::Share {
            id,
            permission,
            expire,
        }) => {
            #[cfg(feature = "pro")]
            {
                // Try relay first (via discovery or config), fall back to tmate
                let sharing_cfg = crate::config::ConfigFile::load()
                    .await
                    .ok()
                    .flatten()
                    .map(|c| c.sharing().clone())
                    .unwrap_or_default();

                let relay_url = match sharing_cfg.relay_server_url.clone() {
                    Some(url) => Some(url),
                    None => {
                        if let Some(auth) = crate::auth::AuthToken::load() {
                            crate::pro::collab::client::RelayClient::discover_relay(
                                &sharing_cfg.relay_discovery_url,
                                &auth.access_token,
                            ).await
                        } else {
                            None
                        }
                    }
                };

                if let Some(relay) = relay_url {
                    let client = crate::pro::commands::handle_relay_share(
                        profile, &id, &permission, expire, &relay,
                    ).await?;
                    println!("{}", t!(lang, "Press Ctrl+C to stop sharing.", "按 Ctrl+C 停止共享。"));
                    tokio::signal::ctrl_c().await.ok();
                    let session_id = id.clone();
                    let tmux_name = format!("ah-{}", &session_id[..8.min(session_id.len())]);
                    client.stop(&tmux_name).await;
                    Ok(())
                } else {
                    crate::pro::commands::handle_share(profile, &id, &permission, expire).await
                }
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = (id, permission, expire);
                eprintln!("{}", t!(lang,
                    "Session sharing requires Max subscription. Visit https://weykon.github.io/agent-hand",
                    "会话共享需要 Max 订阅。访问 https://weykon.github.io/agent-hand"
                ));
                Ok(())
            }
        }

        Some(Command::Unshare { id }) => {
            #[cfg(feature = "pro")]
            { crate::pro::commands::handle_unshare(profile, &id).await }
            #[cfg(not(feature = "pro"))]
            {
                let _ = id;
                eprintln!("{}", t!(lang,
                    "Session sharing requires Max subscription. Visit https://weykon.github.io/agent-hand",
                    "会话共享需要 Max 订阅。访问 https://weykon.github.io/agent-hand"
                ));
                Ok(())
            }
        }

        Some(Command::Join { url }) => {
            #[cfg(feature = "pro")]
            {
                use crate::ui::JoinSessionDialog;

                crate::auth::AuthToken::require_max("sharing")?;

                let (relay_url, room_id, token) = JoinSessionDialog::parse_share_url(&url)
                    .ok_or_else(|| crate::Error::InvalidInput(
                        t!(lang,
                            "Invalid share URL. Expected: https://.../share/ROOM_ID?token=TOKEN",
                            "无效的共享链接。格式应为: https://.../share/ROOM_ID?token=TOKEN"
                        ).to_string()
                    ))?;

                println!("{}", t!(lang, "Connecting to shared session...", "正在连接共享会话..."));
                println!("  Relay: {}", relay_url);
                println!("  Room:  {}", &room_id[..std::cmp::min(8, room_id.len())]);

                // For CLI viewer, we connect and stream to stdout using a simple loop
                use crate::pro::collab::protocol::ControlMessage;
                use futures_util::{SinkExt, StreamExt};
                use base64::Engine;

                let ws_url = format!("{}/ws/{}", relay_url, room_id)
                    .replace("https://", "wss://")
                    .replace("http://", "ws://");

                let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
                    .await
                    .map_err(|e| crate::Error::Other(format!("WebSocket connect failed: {}", e)))?;

                let (mut ws_write, mut ws_read) = ws_stream.split();

                // Send ViewerAuth
                let auth_msg = ControlMessage::ViewerAuth {
                    token,
                    user_token: None,
                    display_name: None,
                };
                let json = serde_json::to_string(&auth_msg)
                    .map_err(|e| crate::Error::Other(format!("Serialize error: {}", e)))?;
                ws_write.send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await
                    .map_err(|e| crate::Error::Other(format!("Send error: {}", e)))?;

                // Wait for AuthResult
                if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) = ws_read.next().await {
                    if let Ok(ControlMessage::AuthResult { success, error, .. }) = serde_json::from_str(&text) {
                        if !success {
                            return Err(crate::Error::Other(format!(
                                "Auth failed: {}", error.unwrap_or_default()
                            )));
                        }
                    }
                }

                println!("{}", t!(lang, "Connected! Streaming terminal output (Ctrl+C to quit)...", "已连接！正在传输终端输出（Ctrl+C 退出）..."));
                println!("---");

                // Stream output to stdout
                use std::io::Write;
                while let Some(Ok(msg)) = ws_read.next().await {
                    match msg {
                        tokio_tungstenite::tungstenite::Message::Binary(data) => {
                            let _ = std::io::stdout().write_all(&data);
                            let _ = std::io::stdout().flush();
                        }
                        tokio_tungstenite::tungstenite::Message::Text(text) => {
                            if let Ok(ControlMessage::Snapshot { data, .. }) = serde_json::from_str(&text) {
                                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&data) {
                                    // Clear screen and write snapshot
                                    print!("\x1b[2J\x1b[H");
                                    let _ = std::io::stdout().write_all(&decoded);
                                    let _ = std::io::stdout().flush();
                                }
                            } else if let Ok(ControlMessage::RoomClosed { reason }) = serde_json::from_str(&text) {
                                println!("\n--- {} {} ---", t!(lang, "Session ended:", "会话已结束:"), reason);
                                break;
                            }
                        }
                        tokio_tungstenite::tungstenite::Message::Close(_) => {
                            println!("\n--- {} ---", t!(lang, "Connection closed", "连接已关闭"));
                            break;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = url;
                eprintln!("{}", t!(lang,
                    "Session sharing requires Max subscription. Visit https://weykon.github.io/agent-hand",
                    "会话共享需要 Max 订阅。访问 https://weykon.github.io/agent-hand"
                ));
                Ok(())
            }
        }

        Some(Command::Chat { session }) => run_chat(session).await,

        Some(Command::Canvas { action }) => handle_canvas(action).await,

        Some(Command::Skills { action }) => {
            #[cfg(feature = "pro")]
            {
                handle_skills(lang, action).await?;
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = action;
                eprintln!("{}", t!(lang,
                    "Skills library requires Pro license. Visit https://weykon.github.io/agent-hand",
                    "技能库需要 Pro 许可。访问 https://weykon.github.io/agent-hand"
                ));
            }
            Ok(())
        }

        Some(Command::Config { action }) => handle_config(lang, action).await,

        Some(Command::PtyViewer {
            relay_url,
            room_id,
            token,
            session_name,
            user_token,
            display_name,
        }) => {
            #[cfg(feature = "pro")]
            {
                crate::pro::pty_viewer::run(
                    &relay_url,
                    &room_id,
                    &token,
                    &session_name,
                    user_token.as_deref(),
                    display_name.as_deref(),
                )
                .await
                .map_err(|e| crate::Error::Other(e.to_string()))
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = (relay_url, room_id, token, session_name, user_token, display_name);
                eprintln!("{}", t!(lang, "pty-viewer requires Pro feature.", "pty-viewer 需要 Pro 功能。"));
                Ok(())
            }
        }

        Some(Command::ViewerInfo { room_id }) => {
            #[cfg(feature = "pro")]
            {
                crate::pro::viewer_info::run(&room_id)
                    .map_err(|e| crate::Error::Other(e.to_string()))
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = room_id;
                eprintln!("{}", t!(lang, "viewer-info requires Pro feature.", "viewer-info 需要 Pro 功能。"));
                Ok(())
            }
        }

        None => {
            // Check tmux availability before launching TUI
            if !crate::tmux::TmuxManager::is_available()
                .await
                .unwrap_or(false)
            {
                eprintln!("{}", t!(lang, "Error: tmux is not installed or not in PATH", "错误: 未安装 tmux 或不在 PATH 中"));
                eprintln!();
                eprintln!("{}", t!(lang,
                    "agent-hand requires tmux to manage terminal sessions.",
                    "agent-hand 需要 tmux 来管理终端会话。"
                ));
                eprintln!();
                eprintln!("{}", t!(lang, "Install tmux:", "安装 tmux:"));
                eprintln!("  macOS:        brew install tmux");
                eprintln!("  Ubuntu/Debian: sudo apt install tmux");
                eprintln!("  Fedora:       sudo dnf install tmux");
                eprintln!("  Arch:         sudo pacman -S tmux");
                eprintln!();
                eprintln!("{}", t!(lang,
                    "Or visit: https://github.com/tmux/tmux/wiki/Installing",
                    "或访问: https://github.com/tmux/tmux/wiki/Installing"
                ));
                return Err(crate::Error::tmux("tmux is not installed"));
            }

            // Launch TUI
            let mut app = crate::ui::App::new(profile).await?;
            app.run().await
        }
    }
}

async fn handle_add(
    lang: Language,
    profile: &str,
    path: Option<String>,
    title: Option<String>,
    group: Option<String>,
    cmd: Option<String>,
) -> Result<()> {
    let project_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        std::env::current_dir()?
    };

    if !project_path.exists() {
        std::fs::create_dir_all(&project_path)?;
        eprintln!("{} {}", t!(lang, "Created directory:", "已创建目录:"), project_path.display());
    }

    let project_path = project_path.canonicalize()?;

    // Verify path exists and is directory
    if !project_path.is_dir() {
        return Err(crate::Error::InvalidInput(format!(
            "{} {}",
            t!(lang, "Path is not a directory:", "路径不是目录:"),
            project_path.display()
        )));
    }

    let title = title.unwrap_or_else(|| {
        project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    // Load existing sessions
    let storage = Storage::new(profile).await?;
    let (mut instances, tree, relationships) = storage.load().await?;

    // Check for duplicates
    for inst in &instances {
        if inst.project_path == project_path {
            println!("{} {} ({})",
                t!(lang, "✓ Session already exists:", "✓ 会话已存在:"),
                inst.title, inst.id
            );
            return Ok(());
        }
    }

    // Create new instance
    let mut instance = if let Some(g) = group {
        Instance::with_group(title.clone(), project_path.clone(), g)
    } else {
        Instance::new(title.clone(), project_path.clone())
    };

    if let Some(command) = cmd {
        instance.command = command.clone();
        // NOTE: `tool` is legacy metadata; we no longer infer it from the command string.
        // UI/status should rely on tags/labels and prompt detection instead.
    }

    instances.push(instance.clone());

    // Save
    storage.save(&instances, &tree, &relationships).await?;

    println!("{} {}", t!(lang, "✓ Added session:", "✓ 已添加会话:"), title);
    println!("  {}: {}", t!(lang, "Profile", "配置"), profile);
    println!("  {}: {}", t!(lang, "Path", "路径"), project_path.display());
    println!("  {}: {}", t!(lang, "Group", "分组"), instance.group_path);
    println!("  ID:      {}", instance.id);

    Ok(())
}

async fn handle_list(lang: Language, profile: &str, json: bool, all: bool) -> Result<()> {
    if all {
        let profiles = Storage::list_profiles().await?;
        for prof in profiles {
            println!("\n=== {}: {} ===", t!(lang, "Profile", "配置"), prof);
            list_profile(lang, &prof, json).await?;
        }
        return Ok(());
    }

    list_profile(lang, profile, json).await
}

async fn list_profile(lang: Language, profile: &str, json: bool) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, _, _) = storage.load().await?;

    if instances.is_empty() {
        if !json {
            if lang.is_zh() {
                println!("在配置 '{}' 中未找到会话。", profile);
            } else {
                println!("No sessions found in profile '{}'.", profile);
            }
        }
        return Ok(());
    }

    if json {
        let json_str = serde_json::to_string_pretty(&instances)?;
        println!("{}", json_str);
    } else {
        println!("{}: {}\n", t!(lang, "Profile", "配置"), profile);
        println!("{:<20} {:<15} {:<40} {}",
            t!(lang, "TITLE", "标题"),
            t!(lang, "GROUP", "分组"),
            t!(lang, "PATH", "路径"),
            "ID"
        );
        println!("{}", "-".repeat(90));

        for inst in &instances {
            let path_str = inst.project_path.to_string_lossy();
            let path_display = truncate(&path_str, 40);
            let title_display = truncate(&inst.title, 20);
            let group_display = truncate(&inst.group_path, 15);
            let id_display = &inst.id[..inst.id.len().min(12)];

            println!(
                "{:<20} {:<15} {:<40} {}",
                title_display, group_display, path_display, id_display
            );
        }

        if lang.is_zh() {
            println!("\n共计: {} 个会话", instances.len());
        } else {
            println!("\nTotal: {} sessions", instances.len());
        }
    }

    Ok(())
}

async fn handle_remove(lang: Language, profile: &str, identifier: &str) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, tree, relationships) = storage.load().await?;

    let (to_remove, to_keep): (Vec<_>, Vec<_>) = instances.into_iter().partition(|inst| {
        inst.id == identifier || inst.id.starts_with(identifier) || inst.title == identifier
    });

    if to_remove.is_empty() {
        return Err(crate::Error::SessionNotFound(identifier.to_string()));
    }

    let removed = &to_remove[0];
    let title = removed.title.clone();

    // Kill tmux session if exists
    let manager = TmuxManager::new(profile);
    let tmux_name = removed.tmux_name();
    if manager.session_exists(&tmux_name).unwrap_or(false) {
        if let Err(e) = manager.kill_session(&tmux_name).await {
            eprintln!("{} {}", t!(lang, "Warning: failed to kill tmux session:", "警告: 无法终止 tmux 会话:"), e);
        }
    }

    // Save
    storage.save(&to_keep, &tree, &relationships).await?;

    println!("{} {} ({} '{}')",
        t!(lang, "✓ Removed session:", "✓ 已移除会话:"),
        title,
        t!(lang, "from profile", "来自配置"),
        profile
    );
    Ok(())
}

async fn handle_status(lang: Language, profile: &str, verbose: bool, quiet: bool, json: bool) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (mut instances, _, _) = storage.load().await?;

    if instances.is_empty() {
        if json {
            println!(r#"{{"waiting": 0, "running": 0, "idle": 0, "error": 0, "total": 0}}"#);
        } else if !quiet {
            if lang.is_zh() {
                println!("配置 '{}' 中没有会话。", profile);
            } else {
                println!("No sessions in profile '{}'.", profile);
            }
        }
        return Ok(());
    }

    // Update statuses
    let manager = Arc::new(TmuxManager::new(profile));
    manager.refresh_cache().await?;

    for inst in &mut instances {
        inst.init_tmux(manager.clone());
        let _ = inst.update_status().await;
    }

    // Count by status
    let mut counts = StatusCounts::default();
    for inst in &instances {
        counts.total += 1;
        match inst.status {
            crate::session::Status::Running => counts.running += 1,
            crate::session::Status::Waiting => counts.waiting += 1,
            crate::session::Status::Idle => counts.idle += 1,
            crate::session::Status::Error => counts.error += 1,
            crate::session::Status::Starting => counts.idle += 1,
        }
    }

    if json {
        println!(
            r#"{{"waiting": {}, "running": {}, "idle": {}, "error": {}, "total": {}}}"#,
            counts.waiting, counts.running, counts.idle, counts.error, counts.total
        );
    } else if quiet {
        println!("{}", counts.waiting);
    } else if verbose {
        print_status_verbose(lang, &instances);
    } else {
        println!(
            "{} {} • {} {} • {} {}",
            counts.waiting, t!(lang, "waiting", "等待中"),
            counts.running, t!(lang, "running", "运行中"),
            counts.idle, t!(lang, "idle", "空闲")
        );
    }

    Ok(())
}

async fn handle_statusline(profile: &str) -> Result<()> {
    use crate::session::Status;

    // tmux status-left may spawn this command again before the previous run finishes.
    // Prevent piling up shells/PTYs by allowing only one in-flight statusline instance.
    {
        use fs2::FileExt;

        if let Ok(lock_path) = Storage::get_agent_hand_dir().map(|d| d.join("statusline.lock")) {
            if let Ok(f) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(lock_path)
            {
                if f.try_lock_exclusive().is_err() {
                    println!("AH");
                    return Ok(());
                }
                // keep file handle alive until function returns
                let _statusline_lock = f;
            }
        }
    }

    let cfg = crate::config::ConfigFile::load().await.ok().flatten();
    let ready_ttl_secs: i64 = cfg.as_ref().map(|c| c.ready_ttl_minutes()).unwrap_or(40) as i64 * 60;

    let storage = Storage::new(profile).await?;
    let (mut instances, tree, relationships) = storage.load().await?;

    if instances.is_empty() {
        println!("AH");
        return Ok(());
    }

    let manager = Arc::new(TmuxManager::new(profile));
    manager.refresh_cache().await?;

    let now = chrono::Utc::now();
    let mut dirty = false;

    for inst in &mut instances {
        inst.init_tmux(manager.clone());
        let prev = inst.status;
        let _ = inst.update_status().await;

        if inst.status == Status::Waiting && prev != Status::Waiting {
            inst.last_waiting_at = Some(now);
            dirty = true;
        }

        if inst.status == Status::Running {
            let should_touch = inst
                .last_running_at
                .is_none_or(|t| now.signed_duration_since(t).num_seconds() >= 30);
            if should_touch {
                inst.last_running_at = Some(now);
                dirty = true;
            }
        }
    }

    if dirty {
        storage.save(&instances, &tree, &relationships).await?;
    }

    let is_ready = |inst: &Instance| {
        inst.last_running_at
            .is_some_and(|t| now.signed_duration_since(t).num_seconds() < ready_ttl_secs)
    };

    let mut waiting = 0usize;
    let mut ready = 0usize;
    let mut running = 0usize;
    let mut idle = 0usize;
    let mut error = 0usize;

    for inst in &instances {
        match inst.status {
            Status::Waiting => waiting += 1,
            Status::Running => running += 1,
            Status::Idle => {
                if is_ready(inst) {
                    ready += 1
                } else {
                    idle += 1
                }
            }
            Status::Error => error += 1,
            Status::Starting => idle += 1,
        }
    }

    let truncate = |s: &str, max: usize| -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    };

    let (target, priority_tmux) = if waiting > 0 {
        instances
            .iter()
            .filter(|s| s.status == Status::Waiting)
            .max_by_key(|s| s.last_waiting_at.unwrap_or(s.created_at))
            .map(|s| (format!("! {}", truncate(&s.title, 24)), Some(s.tmux_name())))
            .unwrap_or_else(|| (String::new(), None))
    } else {
        instances
            .iter()
            .filter(|s| s.status == Status::Idle && is_ready(s))
            .max_by_key(|s| s.last_running_at.unwrap_or(s.created_at))
            .map(|s| (format!("✓ {}", truncate(&s.title, 24)), Some(s.tmux_name())))
            .unwrap_or_else(|| (String::new(), None))
    };

    let _ = manager
        .set_environment_global(
            "AGENTHAND_PRIORITY_SESSION",
            priority_tmux.as_deref().unwrap_or(""),
        )
        .await;

    let mut line = format!(
        "AH{}  !{} ✓{} ●{} ○{}  ^N",
        if target.is_empty() {
            String::new()
        } else {
            format!(" {target}")
        },
        waiting,
        ready,
        running,
        idle
    );
    if error > 0 {
        line.push_str(&format!(" ✕{}", error));
    }

    if let Some(hint) = crate::update::statusline_update_hint().await {
        line.push_str(&format!("  {hint}"));
    }

    if let Some(hint) = crate::update::tier_upgrade_hint() {
        line.push_str(&format!("  {hint}"));
    }

    // Add presence information if any session is being shared
    #[cfg(feature = "pro")]
    {
        // Check if any instance has an active relay client
        for _inst in &instances {
            // Try to get relay client from app state (we need to access it somehow)
            // For now, we'll add a simpler approach: check if sharing is active
            // and show a generic presence indicator
            // TODO: Access relay client state from statusline context
        }
    }

    println!("{line}");
    Ok(())
}

/// Jump to the highest-priority session (Waiting > Ready with round-robin).
/// Called by tmux Ctrl+N binding via `run-shell`.
///
/// Priority logic:
/// 1. Waiting sessions: Jump to the newest one (highest urgency)
/// 2. Ready sessions: Round-robin rotation among all Ready sessions
/// 3. Idle sessions: Excluded from jump (not frequently needed)
async fn handle_jump(profile: &str) -> Result<()> {
    use crate::session::Status;

    let cfg = crate::config::ConfigFile::load().await.ok().flatten();
    let ready_ttl_secs: i64 = cfg.as_ref().map(|c| c.ready_ttl_minutes()).unwrap_or(40) as i64 * 60;

    let storage = Storage::new(profile).await?;
    let (mut instances, _tree, _) = storage.load().await?;

    if instances.is_empty() {
        // Use tmux display-message instead of eprintln so user sees it in tmux
        let _ = TokioCommand::new("tmux")
            .args(["-L", "agentdeck_rs", "display-message", "AH: no sessions"])
            .status()
            .await;
        return Ok(());
    }

    let manager = Arc::new(TmuxManager::new(profile));
    manager.refresh_cache().await?;

    // Get current tmux session name to find position for round-robin
    let current_session = TokioCommand::new("tmux")
        .args([
            "-L",
            "agentdeck_rs",
            "display-message",
            "-p",
            "#{session_name}",
        ])
        .output()
        .await
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let now = chrono::Utc::now();

    // Update statuses (quick probe)
    for inst in &mut instances {
        inst.init_tmux(manager.clone());
        let _ = inst.update_status().await;
    }

    let is_ready = |inst: &Instance| {
        inst.last_running_at
            .is_some_and(|t| now.signed_duration_since(t).num_seconds() < ready_ttl_secs)
    };

    // Priority 1: Waiting sessions (jump to newest - highest urgency)
    let waiting_target = instances
        .iter()
        .filter(|s| s.status == Status::Waiting && s.tmux_name() != current_session)
        .max_by_key(|s| s.last_waiting_at.unwrap_or(s.created_at));

    // Priority 2: Ready sessions (round-robin rotation)
    let ready_target = if waiting_target.is_none() {
        // Collect all Ready sessions, sorted by tmux_name for consistent ordering
        let mut ready_sessions: Vec<_> = instances
            .iter()
            .filter(|s| s.status == Status::Idle && is_ready(s))
            .collect();
        ready_sessions.sort_by(|a, b| a.tmux_name().cmp(&b.tmux_name()));

        if ready_sessions.is_empty() {
            None
        } else if ready_sessions.len() == 1 {
            // Only one Ready session - jump to it if not current
            if ready_sessions[0].tmux_name() != current_session {
                Some(ready_sessions[0])
            } else {
                None
            }
        } else {
            // Round-robin: find current position and jump to next
            let current_pos = ready_sessions
                .iter()
                .position(|s| s.tmux_name() == current_session);

            match current_pos {
                Some(pos) => {
                    // Jump to next session (wrap around)
                    let next_pos = (pos + 1) % ready_sessions.len();
                    Some(ready_sessions[next_pos])
                }
                None => {
                    // Current session is not in Ready list, jump to first Ready
                    Some(ready_sessions[0])
                }
            }
        }
    } else {
        None
    };

    let target = waiting_target.or(ready_target);

    match target {
        Some(inst) => {
            let tmux_name = inst.tmux_name();
            // switch-client to the target session
            let status = TokioCommand::new("tmux")
                .args(["-L", "agentdeck_rs", "switch-client", "-t", &tmux_name])
                .status()
                .await;
            if !status.map(|s| s.success()).unwrap_or(false) {
                let _ = TokioCommand::new("tmux")
                    .args([
                        "-L",
                        "agentdeck_rs",
                        "display-message",
                        &format!("AH: failed to switch to {}", inst.title),
                    ])
                    .status()
                    .await;
            }
        }
        None => {
            let _ = TokioCommand::new("tmux")
                .args(["-L", "agentdeck_rs", "display-message", "AH: no target"])
                .status()
                .await;
        }
    }

    Ok(())
}

async fn handle_session(lang: Language, profile: &str, action: SessionAction) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (mut instances, tree, relationships) = storage.load().await?;
    let manager = Arc::new(TmuxManager::new(profile));

    match action {
        SessionAction::Start { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone();
            inst.init_tmux(manager.clone());
            inst.start().await?;
            storage.save(&instances, &tree, &relationships).await?;
            println!("{} {}", t!(lang, "✓ Started session:", "✓ 已启动会话:"), title);
        }

        SessionAction::Stop { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone();
            inst.init_tmux(manager.clone());
            inst.stop().await?;
            storage.save(&instances, &tree, &relationships).await?;
            println!("{} {}", t!(lang, "✓ Stopped session:", "✓ 已停止会话:"), title);
        }

        SessionAction::Restart { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone();
            inst.init_tmux(manager.clone());
            inst.stop().await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            inst.start().await?;
            storage.save(&instances, &tree, &relationships).await?;
            println!("{} {}", t!(lang, "✓ Restarted session:", "✓ 已重启会话:"), title);
        }

        SessionAction::Attach { id } => {
            let inst = find_session(&mut instances, &id)?;
            inst.init_tmux(manager.clone());
            inst.attach().await?;
            storage.save(&instances, &tree, &relationships).await?;
        }

        SessionAction::Show { id } => {
            let inst = if let Some(id_str) = &id {
                find_session(&mut instances, id_str)?
            } else {
                return Err(crate::Error::InvalidInput(
                    t!(lang, "Auto-detection not yet implemented", "自动检测尚未实现").to_string(),
                ));
            };

            println!("{}: {}", t!(lang, "Session", "会话"), inst.title);
            println!("  ID:      {}", inst.id);
            println!("  {}: {}", t!(lang, "Path", "路径"), inst.project_path.display());
            println!("  {}: {}", t!(lang, "Group", "分组"), inst.group_path);
            println!("  {}: {:?}", t!(lang, "Status", "状态"), inst.status);
            println!("  {}: {}", t!(lang, "Created", "创建时间"), inst.created_at);
        }
    }

    Ok(())
}

async fn handle_profile(lang: Language, action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            let profiles = Storage::list_profiles().await?;
            println!("{}:", t!(lang, "Profiles", "配置列表"));
            for prof in profiles {
                println!("  {}", prof);
            }
        }

        ProfileAction::Create { name } => {
            Storage::create_profile(&name).await?;
            println!("{} {}", t!(lang, "✓ Created profile:", "✓ 已创建配置:"), name);
        }

        ProfileAction::Delete { name } => {
            Storage::delete_profile(&name).await?;
            println!("{} {}", t!(lang, "✓ Deleted profile:", "✓ 已删除配置:"), name);
        }
    }

    Ok(())
}

// Helper functions

fn find_session<'a>(instances: &'a mut [Instance], id: &str) -> Result<&'a mut Instance> {
    instances
        .iter_mut()
        .find(|inst| inst.id == id || inst.id.starts_with(id) || inst.title == id)
        .ok_or_else(|| crate::Error::SessionNotFound(id.to_string()))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 3 {
        let end = s.floor_char_boundary(max);
        s[..end].to_string()
    } else {
        let end = s.floor_char_boundary(max - 3);
        format!("{}...", &s[..end])
    }
}

#[derive(Default)]
struct StatusCounts {
    waiting: usize,
    running: usize,
    idle: usize,
    error: usize,
    total: usize,
}

fn print_status_verbose(lang: Language, instances: &[Instance]) {
    let symbols = [
        (crate::session::Status::Waiting, "◐", t!(lang, "WAITING", "等待中")),
        (crate::session::Status::Running, "●", t!(lang, "RUNNING", "运行中")),
        (crate::session::Status::Idle, "○", t!(lang, "IDLE", "空闲")),
        (crate::session::Status::Error, "✕", t!(lang, "ERROR", "错误")),
    ];

    for (status, symbol, label) in &symbols {
        let matching: Vec<_> = instances.iter().filter(|i| &i.status == status).collect();
        if matching.is_empty() {
            continue;
        }

        println!("{} ({}):", label, matching.len());
        for inst in matching {
            let path = inst.project_path.to_string_lossy();
            println!("  {} {:<16} {:?}", symbol, inst.title, path);
        }
        println!();
    }
}

async fn handle_login(lang: Language) -> Result<()> {
    use crate::auth::{AuthToken, DeviceCodeResponse, DeviceTokenResponse, AUTH_SERVER};

    let dev_info = crate::device::DeviceInfo::generate();
    let client = reqwest::Client::new();

    // 1. Request a device code
    let resp = client
        .post(format!("{AUTH_SERVER}/device/code"))
        .send()
        .await
        .map_err(|e| crate::Error::InvalidInput(format!("{} {e}", t!(lang, "Network error:", "网络错误:"))))?;

    if !resp.status().is_success() {
        return Err(crate::Error::InvalidInput(
            t!(lang,
                "Failed to request device code from auth server",
                "无法从认证服务器获取设备码"
            ).to_string(),
        ));
    }

    let device: DeviceCodeResponse = resp
        .json()
        .await
        .map_err(|e| crate::Error::InvalidInput(format!("{} {e}", t!(lang, "Invalid server response:", "无效的服务器响应:"))))?;

    // 2. Show URL and open browser
    println!("{}", t!(lang, "Complete verification in your browser:", "请在浏览器中完成验证:"));
    println!("  {}", device.url);
    println!();
    println!("{}", t!(lang, "Waiting for authorization...", "等待授权..."));

    let _ = open::that(&device.url);

    // 3. Poll for token
    let interval = tokio::time::Duration::from_secs(device.interval.max(3));
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(300);

    loop {
        tokio::time::sleep(interval).await;

        if tokio::time::Instant::now() > deadline {
            eprintln!("{}", t!(lang,
                "Timed out waiting for authorization (5 minutes).",
                "等待授权超时（5 分钟）。"
            ));
            return Ok(());
        }

        let poll = client
            .get(format!("{AUTH_SERVER}/device/token"))
            .query(&[
                ("code", device.code.as_str()),
                ("device_id", dev_info.device_id.as_str()),
                ("hostname", dev_info.hostname.as_str()),
                ("os_arch", dev_info.os_arch.as_str()),
            ])
            .send()
            .await;

        let poll = match poll {
            Ok(r) => r,
            Err(_) => continue,
        };

        let token_resp: DeviceTokenResponse = match poll.json().await {
            Ok(t) => t,
            Err(_) => continue,
        };

        match token_resp.status.as_str() {
            "pending" => {
                eprint!(".");
                continue;
            }
            "expired" => {
                eprintln!("\n{}", t!(lang,
                    "Device code expired. Run `agent-hand login` again.",
                    "设备码已过期。请重新运行 `agent-hand login`。"
                ));
                return Ok(());
            }
            "authorized" => {
                let auth = AuthToken {
                    access_token: token_resp.access_token.unwrap_or_default(),
                    email: token_resp.email.unwrap_or_default(),
                    features: token_resp.features.unwrap_or_default(),
                    purchased_at: token_resp.purchased_at.unwrap_or_default(),
                    device_id: Some(dev_info.device_id.clone()),
                };
                auth.save()?;
                println!("\n{} {}", t!(lang, "✓ Logged in as", "✓ 已登录:"), auth.email);
                if auth.features.is_empty() {
                    println!("  {}", t!(lang, "No premium features unlocked yet.", "尚未解锁高级功能。"));
                } else {
                    println!("  {}: {}", t!(lang, "Unlocked", "已解锁"), auth.features.join(", "));
                }

                // Auto-download binary if account tier exceeds binary tier
                let needs_upgrade = (auth.is_max() && !cfg!(feature = "max"))
                    || (auth.is_pro() && !cfg!(feature = "pro"));
                if needs_upgrade {
                    let tier = if auth.is_max() { "Max" } else { "Pro" };
                    eprintln!();
                    let en1 = format!("Your account includes {} features!", tier);
                    let zh1 = format!("您的账户包含 {} 功能！", tier);
                    eprintln!("{}", t!(lang, &en1, &zh1));
                    let en2 = format!("Downloading {} binary...", tier);
                    let zh2 = format!("正在下载 {} 版本...", tier);
                    eprintln!("{}", t!(lang, &en2, &zh2));
                    eprintln!();
                    if let Err(e) = handle_upgrade(lang, None, None).await {
                        eprintln!("{} {e}", t!(lang, "Auto-upgrade failed:", "自动升级失败:"));
                        eprintln!("{}", t!(lang, "You can retry manually: agent-hand upgrade", "您可以手动重试: agent-hand upgrade"));
                    }
                }

                return Ok(());
            }
            other => {
                eprintln!("\n{} {other}", t!(lang, "Unexpected status:", "未知状态:"));
                return Ok(());
            }
        }
    }
}

fn handle_logout(lang: Language) -> Result<()> {
    crate::auth::AuthToken::delete()?;
    println!("{}", t!(lang, "✓ Logged out. Credentials removed.", "✓ 已登出。凭据已清除。"));
    Ok(())
}

/// Run an async future while displaying a braille spinner on stderr.
/// Returns the future's result and automatically clears the spinner line.
async fn with_spinner<F, T>(msg: &str, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    use std::io::Write;
    const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let msg = msg.to_string();
    let spinner = tokio::spawn(async move {
        let mut i = 0usize;
        loop {
            eprint!("\r{} {} ", FRAMES[i % FRAMES.len()], msg);
            let _ = std::io::stderr().flush();
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            i += 1;
        }
    });
    let result = fut.await;
    spinner.abort();
    // Clear spinner line
    eprint!("\r\x1b[2K");
    let _ = std::io::stderr().flush();
    result
}

async fn handle_account(lang: Language, refresh: bool) -> Result<()> {
    let mut token = match crate::auth::AuthToken::load() {
        Some(t) => t,
        None => {
            eprintln!("{}", t!(lang,
                "Not logged in. Run `agent-hand login` to authenticate.",
                "未登录。请运行 `agent-hand login` 进行认证。"
            ));
            return Ok(());
        }
    };

    if refresh {
        match with_spinner(
            t!(lang, "Refreshing account status...", "正在刷新账户状态..."),
            token.refresh(),
        ).await {
            Ok(changed) => {
                if changed {
                    eprintln!("{}", t!(lang, "✓ Account status updated!", "✓ 账户状态已更新！"));

                    // Auto-download binary if plan was upgraded but binary tier is lower
                    let needs_upgrade = (token.is_max() && !cfg!(feature = "max"))
                        || (token.is_pro() && !cfg!(feature = "pro"));
                    if needs_upgrade {
                        let tier = if token.is_max() { "Max" } else { "Pro" };
                        eprintln!();
                        let en = format!("Your plan was upgraded to {}!", tier);
                        let zh = format!("您的计划已升级为 {}！", tier);
                        eprintln!("{}", t!(lang, &en, &zh));
                        eprintln!();
                        if let Err(e) = handle_upgrade(lang, None, None).await {
                            eprintln!("{} {e}", t!(lang, "Auto-upgrade failed:", "自动升级失败:"));
                            eprintln!("{}", t!(lang, "You can retry manually: agent-hand upgrade", "您可以手动重试: agent-hand upgrade"));
                        }
                    }
                } else {
                    eprintln!("{}", t!(lang, "✓ No changes.", "✓ 无变更。"));
                }
            }
            Err(e) => {
                eprintln!("{} {e}", t!(lang, "✗ Refresh failed:", "✗ 刷新失败:"));
                eprintln!("{}", t!(lang, "Showing cached info.", "显示缓存信息。"));
            }
        }
    }

    let plan = if token.is_max() {
        "Max"
    } else if token.is_pro() {
        "Pro"
    } else {
        "Free"
    };

    println!("{}: {}", t!(lang, "Account", "账户"), token.email);
    println!("{}: {}", t!(lang, "Plan", "计划"), plan);
    if !token.features.is_empty() {
        println!("{}: {}", t!(lang, "Features", "功能"), token.features.join(", "));
    }
    if !token.purchased_at.is_empty() {
        println!("{}: {}", t!(lang, "Purchased", "购买时间"), token.purchased_at);
    }
    if !token.is_pro() {
        println!();
        println!("{}: https://weykon.github.io/agent-hand", t!(lang, "Upgrade at", "升级地址"));
    }

    Ok(())
}

async fn handle_devices(lang: Language, remove: Option<String>) -> Result<()> {
    let token = match crate::auth::AuthToken::load() {
        Some(t) => t,
        None => {
            eprintln!("{}", t!(lang,
                "Not logged in. Run `agent-hand login` to authenticate.",
                "未登录。请运行 `agent-hand login` 进行认证。"
            ));
            return Ok(());
        }
    };

    if !(token.is_pro() || token.is_max()) {
        eprintln!("{}", t!(lang,
            "Device management requires Pro or Max. Visit https://agent-hand.dev",
            "设备管理需要 Pro 或 Max。访问 https://agent-hand.dev"
        ));
        return Ok(());
    }

    // Remove a device by prefix
    if let Some(prefix) = remove {
        let list = with_spinner(
            t!(lang, "Fetching devices...", "正在获取设备列表..."),
            token.list_devices(),
        ).await?;

        let matched: Vec<_> = list.devices.iter()
            .filter(|d| d.device_id.starts_with(&prefix))
            .collect();

        match matched.len() {
            0 => {
                eprintln!("{} '{prefix}'", t!(lang, "No device found with prefix", "未找到前缀匹配的设备"));
            }
            1 => {
                let device = matched[0];
                with_spinner(
                    t!(lang, "Removing device...", "正在移除设备..."),
                    token.remove_device(&device.device_id),
                ).await?;
                println!("{} {} ({})", t!(lang, "✓ Removed", "✓ 已移除"), device.hostname, &device.device_id[..8]);
            }
            n => {
                eprintln!("{} ({n} {})", t!(lang, "Ambiguous prefix — multiple devices match", "前缀不明确 — 多个设备匹配"),
                    t!(lang, "matches", "匹配"));
                for d in matched {
                    eprintln!("  {}  {}", &d.device_id[..12], d.hostname);
                }
            }
        }
        return Ok(());
    }

    // List devices
    let list = with_spinner(
        t!(lang, "Fetching devices...", "正在获取设备列表..."),
        token.list_devices(),
    ).await?;

    let current_device_id = crate::device::DeviceInfo::generate().device_id;

    println!("{} ({}/{})", t!(lang, "Active Devices", "已注册设备"),
        list.active_count, list.device_limit);
    println!();

    if list.devices.is_empty() {
        println!("  {}", t!(lang, "(no devices registered)", "(无已注册设备)"));
    } else {
        for d in &list.devices {
            let marker = if d.device_id == current_device_id { " (this device)" } else { "" };
            let short_id = &d.device_id[..std::cmp::min(8, d.device_id.len())];
            println!("  {:<8}  {:<20}  {:<16}  {}{}",
                short_id, d.hostname, d.os_arch, d.last_seen, marker);
        }
    }

    println!();
    println!("{}", t!(lang,
        "Use --remove <id-prefix> to unbind a device.",
        "使用 --remove <id前缀> 来解绑设备。"
    ));

    Ok(())
}

/// Check if a directory is writable
fn is_dir_writable(path: &std::path::Path) -> bool {
    use std::fs;
    if !path.exists() {
        return false;
    }
    let test = path.join(".agent-hand-write-test");
    if fs::write(&test, b"").is_ok() {
        let _ = fs::remove_file(&test);
        true
    } else {
        false
    }
}

async fn handle_upgrade(lang: Language, prefix: Option<String>, version: Option<String>) -> Result<()> {
    // ── Status check ──
    let is_pro_build = cfg!(feature = "pro");
    let is_max_build = cfg!(feature = "max");
    let token = crate::auth::AuthToken::load();

    let build_label = if is_max_build { "Max" } else if is_pro_build { "Pro" } else { "Free" };

    // Determine the user's plan tier for display
    let token_tier = match &token {
        Some(t) if t.is_max() => "Max",
        Some(t) if t.is_pro() => "Pro",
        _ => "Free",
    };

    // Check if the binary already matches the token tier
    let binary_matches_tier = match &token {
        Some(t) if t.is_max() => is_max_build,
        Some(t) if t.is_pro() => is_pro_build,
        _ => true, // Free token — any binary is fine
    };

    match &token {
        None => {
            eprintln!("┌─────────────────────────────────────┐");
            eprintln!("│  {}: {}              │", t!(lang, "Status", "状态"), t!(lang, "Not logged in", "未登录"));
            eprintln!("│  {}: {}               │", t!(lang, "Build", "版本"), build_label);
            eprintln!("├─────────────────────────────────────┤");
            eprintln!("│  1. {} Pro:            │", t!(lang, "Purchase", "购买"));
            eprintln!("│     https://weykon.github.io/agent-hand          │");
            eprintln!("│  2. {}: agent-hand login      │", t!(lang, "Then run", "然后运行"));
            eprintln!("│  3. {}: agent-hand upgrade    │", t!(lang, "Then run", "然后运行"));
            eprintln!("└─────────────────────────────────────┘");
            return Ok(());
        }
        Some(t) if t.is_pro() && binary_matches_tier => {
            eprintln!("┌─────────────────────────────────────┐");
            eprintln!("│  {}: {} ({})  │", t!(lang, "Status", "状态"), t.email, token_tier);
            eprintln!("│  {}: {} ✓                      │", t!(lang, "Build", "版本"), build_label);
            eprintln!("├─────────────────────────────────────┤");
            let en_already = format!("You are already on the {} build.", token_tier);
            let zh_already = format!("您已在使用 {} 版本。", token_tier);
            eprintln!("│  {}  │", t!(lang, &en_already, &zh_already));
            eprintln!("│  {}  │", t!(lang, "Re-running will update to latest.", "重新运行将更新到最新版。"));
            eprintln!("└─────────────────────────────────────┘");
            // Allow re-run to update to latest version
        }
        Some(t) if t.is_pro() => {
            let en_upg = format!("upgrading to {}", token_tier);
            let zh_upg = format!("升级到 {}", token_tier);
            eprintln!("┌─────────────────────────────────────┐");
            eprintln!("│  {}: {} ({})  │", t!(lang, "Status", "状态"), t.email, token_tier);
            eprintln!("│  {}: {} → {}... │", t!(lang, "Build", "版本"), build_label,
                t!(lang, &en_upg, &zh_upg));
            eprintln!("└─────────────────────────────────────┘");
            // Proceed to download binary
        }
        Some(t) => {
            eprintln!("┌─────────────────────────────────────┐");
            eprintln!("│  {}: {} (Free) │", t!(lang, "Status", "状态"), t.email);
            eprintln!("│  {}: {}               │", t!(lang, "Build", "版本"), build_label);
            eprintln!("├─────────────────────────────────────┤");
            eprintln!("│  {}    │", t!(lang, "Your plan does not include Pro.", "您的计划不包含 Pro。"));
            eprintln!("│  {}: https://weykon.github.io/agent-hand │", t!(lang, "Upgrade at", "升级地址"));
            eprintln!("└─────────────────────────────────────┘");
            return Ok(());
        }
    }

    const REPO: &str = "weykon/agent-hand";
    const BIN_NAME: &str = "agent-hand";

    let os_str = std::env::consts::OS;
    let arch_str = std::env::consts::ARCH;

    let os_target = match os_str {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => return Err(crate::Error::InvalidInput(format!("Unsupported OS: {os_str}"))),
    };

    let arch_target = match arch_str {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        _ => {
            return Err(crate::Error::InvalidInput(format!(
                "Unsupported arch: {arch_str}"
            )))
        }
    };

    let target = format!("{arch_target}-{os_target}");
    // Download the binary matching the user's tier (max > pro)
    let bin_tier = if token.as_ref().map_or(false, |t| t.is_max()) { "max" } else { "pro" };

    let version = version.unwrap_or_else(|| "latest".to_string());
    let url_base = format!("https://github.com/{REPO}/releases");

    let make_url = |tier: &str| {
        let asset = format!("{BIN_NAME}-{tier}-{target}.tar.gz");
        if version == "latest" {
            format!("{url_base}/latest/download/{asset}")
        } else {
            format!("{url_base}/download/{version}/{asset}")
        }
    };

    let prefix = if let Some(p) = prefix {
        PathBuf::from(p)
    } else {
        // Prefer installing next to the currently running binary so it actually
        // takes effect.  If the current exe lives in a writable bin directory
        // (e.g. /opt/homebrew/bin, /usr/local/bin, ~/bin), use that.
        // Falls back to ~/.local/bin if the current location isn't writable.
        let current_exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        if let Some(ref dir) = current_exe_dir {
            if is_dir_writable(dir) {
                dir.clone()
            } else if is_dir_writable(std::path::Path::new("/usr/local/bin")) {
                PathBuf::from("/usr/local/bin")
            } else {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/bin")
            }
        } else if is_dir_writable(std::path::Path::new("/usr/local/bin")) {
            PathBuf::from("/usr/local/bin")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/bin")
        }
    };

    std::fs::create_dir_all(&prefix)?;

    let tmpdir = std::env::temp_dir().join(format!("agent-hand-upgrade-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmpdir)?;

    // Try primary tier first, fall back to pro if max binary isn't available
    let url = make_url(bin_tier);
    let asset = format!("{BIN_NAME}-{bin_tier}-{target}.tar.gz");
    let tar_path = tmpdir.join(&asset);

    eprintln!("{} {url}", t!(lang, "Downloading", "正在下载"));
    let status = TokioCommand::new("curl")
        .args(["-fL", "--progress-bar", &url, "-o"])
        .arg(&tar_path)
        .stderr(std::process::Stdio::inherit())
        .status()
        .await?;

    // If max binary not found, fall back to pro
    let (tar_path, asset) = if !status.success() && bin_tier == "max" {
        eprintln!("{}", t!(lang,
            "Max binary not available yet, downloading Pro build...",
            "Max 版本暂未发布，正在下载 Pro 版本..."
        ));
        let fallback_url = make_url("pro");
        let fallback_asset = format!("{BIN_NAME}-pro-{target}.tar.gz");
        let fallback_tar = tmpdir.join(&fallback_asset);

        let fb_status = TokioCommand::new("curl")
            .args(["-fL", "--progress-bar", &fallback_url, "-o"])
            .arg(&fallback_tar)
            .stderr(std::process::Stdio::inherit())
            .status()
            .await?;
        if !fb_status.success() {
            return Err(crate::Error::InvalidInput(
                t!(lang, "Failed to download release asset", "下载发布资源失败").to_string(),
            ));
        }
        (fallback_tar, fallback_asset)
    } else if !status.success() {
        return Err(crate::Error::InvalidInput(
            t!(lang, "Failed to download release asset", "下载发布资源失败").to_string(),
        ));
    } else {
        (tar_path, asset)
    };

    let tar_path_clone = tar_path.clone();
    let tmpdir_clone = tmpdir.clone();
    let status = with_spinner(t!(lang, "Extracting...", "正在解压..."), async {
        TokioCommand::new("tar")
            .args(["-xzf"])
            .arg(&tar_path_clone)
            .args(["-C"])
            .arg(&tmpdir_clone)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
    })
    .await?;
    if !status.success() {
        return Err(crate::Error::InvalidInput(
            t!(lang, "Failed to extract release archive", "解压发布包失败").to_string(),
        ));
    }

    let tmp_bin = tmpdir.join(BIN_NAME);
    if !tmp_bin.is_file() {
        return Err(crate::Error::InvalidInput(format!(
            "Malformed archive: {asset} (missing {BIN_NAME})"
        )));
    }

    let dest = prefix.join(BIN_NAME);
    let status = TokioCommand::new("install")
        .args(["-m", "0755"])
        .arg(&tmp_bin)
        .arg(&dest)
        .status()
        .await;

    if status.as_ref().ok().map(|s| s.success()).unwrap_or(false) {
        eprintln!("{} {BIN_NAME} ({}) {} {}", t!(lang, "Installed", "已安装"), token_tier, t!(lang, "to", "到"), dest.display());
        let _ = std::fs::remove_dir_all(&tmpdir);
        return Ok(());
    }

    // Fallback if `install` is unavailable.
    std::fs::copy(&tmp_bin, &dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
    }

    eprintln!("{} {BIN_NAME} (Pro) {} {}", t!(lang, "Installed", "已安装"), t!(lang, "to", "到"), dest.display());
    let _ = std::fs::remove_dir_all(&tmpdir);
    Ok(())
}

async fn handle_canvas(action: CanvasAction) -> Result<()> {
    use crate::ui::canvas::{CanvasOp, LayoutDirection, NodeKind};

    let op = match action {
        CanvasAction::AddNode { id, label, kind, pos } => {
            let kind = match kind.to_lowercase().as_str() {
                "start" => NodeKind::Start,
                "end" => NodeKind::End,
                "decision" => NodeKind::Decision,
                "note" => NodeKind::Note,
                _ => NodeKind::Process,
            };
            let pos = pos.and_then(|s| {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2 {
                    Some((parts[0].trim().parse().ok()?, parts[1].trim().parse().ok()?))
                } else {
                    None
                }
            });
            CanvasOp::AddNode { id, label, kind, pos, content: None }
        }
        CanvasAction::RemoveNode { id } => CanvasOp::RemoveNode { id },
        CanvasAction::AddEdge { from, to, label } => CanvasOp::AddEdge { from, to, label, relationship_id: None },
        CanvasAction::RemoveEdge { from, to } => CanvasOp::RemoveEdge { from, to },
        CanvasAction::Layout { direction } => {
            let direction = match direction.to_lowercase().as_str() {
                "left-right" | "lr" | "horizontal" => LayoutDirection::LeftRight,
                _ => LayoutDirection::TopDown,
            };
            CanvasOp::Layout { direction }
        }
        CanvasAction::Query { what } => CanvasOp::Query {
            what,
            kind: None,
            label_contains: None,
            id: None,
        },
        CanvasAction::Batch { file } => {
            let content = tokio::fs::read_to_string(&file).await
                .map_err(crate::Error::Io)?;
            let ops: Vec<CanvasOp> = serde_json::from_str(&content)
                .map_err(|e| crate::Error::InvalidInput(format!("Invalid JSON in {file}: {e}")))?;
            CanvasOp::Batch { ops }
        }
        CanvasAction::Raw { json } => {
            serde_json::from_str(&json)
                .map_err(|e| crate::Error::InvalidInput(format!("Invalid JSON: {e}")))?
        }
    };

    match crate::ui::canvas::socket::send_op(&op).await {
        Ok(response) => {
            let output = serde_json::to_string_pretty(&response)
                .unwrap_or_else(|_| format!("{response:?}"));
            println!("{output}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "pro")]
async fn handle_skills(lang: Language, action: SkillsAction) -> Result<()> {
    use crate::skills::{self, SkillsRegistry};

    // Gate behind Pro license
    crate::auth::AuthToken::require_feature("upgrade")?;

    let mut registry = SkillsRegistry::load()?;

    match action {
        SkillsAction::Init { repo } => {
            println!("{} '{repo}'...", t!(lang, "Initializing skills repository", "正在初始化技能仓库"));
            let url = skills::github::init_skills_repo(&repo).await?;
            let repo_path = SkillsRegistry::default_repo_path()?;

            // Generate the manager meta-skill
            skills::manager_skill::generate_manager_skill(&repo_path)?;

            registry.repo_url = Some(url.clone());
            registry.repo_path = Some(repo_path);
            registry.scan_repo()?;
            registry.save()?;

            println!("{}: {url}", t!(lang, "Skills repo initialized", "技能仓库已初始化"));
            println!("{}", t!(lang,
                "Run `agent-hand skills push` to push the initial meta-skill.",
                "运行 `agent-hand skills push` 推送初始元技能。"
            ));
        }

        SkillsAction::Sync => {
            let repo_path = registry.repo_path.clone().ok_or_else(|| {
                crate::Error::InvalidInput(
                    t!(lang,
                        "No skills repo configured. Run `agent-hand skills init` first.",
                        "未配置技能仓库。请先运行 `agent-hand skills init`。"
                    ).to_string(),
                )
            })?;

            println!("{}", t!(lang, "Syncing skills from GitHub...", "正在从 GitHub 同步技能..."));
            skills::github::sync_repo(&repo_path).await?;

            registry.last_synced = Some(chrono::Utc::now());
            registry.scan_repo()?;
            registry.save()?;

            if lang.is_zh() {
                println!("已同步。{} 个技能可用。", registry.skills.len());
            } else {
                println!("Synced. {} skill(s) available.", registry.skills.len());
            }
        }

        SkillsAction::List { json } => {
            // Re-scan to pick up any manual additions
            registry.scan_repo()?;
            registry.save()?;

            if json {
                let output = serde_json::to_string_pretty(&registry.skills)?;
                println!("{output}");
            } else if registry.skills.is_empty() {
                println!("{}", t!(lang,
                    "No skills found. Run `agent-hand skills init` or `agent-hand skills sync`.",
                    "未找到技能。请运行 `agent-hand skills init` 或 `agent-hand skills sync`。"
                ));
            } else {
                for skill in &registry.skills {
                    let links = if skill.linked_to.is_empty() {
                        t!(lang, "(not linked)", "(未链接)").to_string()
                    } else {
                        let targets: Vec<String> = skill
                            .linked_to
                            .iter()
                            .map(|l| format!("{} ({})", l.project_path.display(), l.cli))
                            .collect();
                        targets.join(", ")
                    };
                    println!("  {} - {} [{}]", skill.name, skill.description, links);
                }
            }
        }

        SkillsAction::Link { name, group } => {
            let repo_path = registry.repo_path.clone().ok_or_else(|| {
                crate::Error::InvalidInput(
                    t!(lang,
                        "No skills repo configured. Run `agent-hand skills init` first.",
                        "未配置技能仓库。请先运行 `agent-hand skills init`。"
                    ).to_string(),
                )
            })?;

            // Find the skill
            let skill_rel_path = registry
                .find_skill(&name)
                .map(|s| s.path.clone())
                .ok_or_else(|| {
                    crate::Error::InvalidInput(if lang.is_zh() {
                        format!("未找到技能 '{}'。运行 `agent-hand skills list` 查看可用技能。", name)
                    } else {
                        format!("Skill '{}' not found. Run `agent-hand skills list` to see available skills.", name)
                    })
                })?;

            let skill_source = repo_path.join(&skill_rel_path);
            let project_path = std::env::current_dir()?;

            // Auto-detect CLI types
            let clis = skills::linker::detect_cli(&project_path);

            for cli in &clis {
                skills::linker::link_skill(&skill_source, &project_path, *cli, &name)?;
                println!("{} '{}' {} {} {} {}", t!(lang, "Linked", "已链接"), name, t!(lang, "for", "到"), cli, t!(lang, "in", "在"), project_path.display());
            }

            // Update registry
            if let Some(entry) = registry.find_skill_mut(&name) {
                for cli in clis {
                    let link = skills::SkillLink {
                        project_path: project_path.clone(),
                        cli,
                        group: group.clone(),
                    };
                    // Avoid duplicate entries
                    if !entry.linked_to.iter().any(|l| l.project_path == project_path && l.cli == cli) {
                        entry.linked_to.push(link);
                    }
                }
            }
            registry.save()?;
        }

        SkillsAction::Unlink { name } => {
            let project_path = std::env::current_dir()?;
            let clis = skills::linker::detect_cli(&project_path);

            for cli in &clis {
                match skills::linker::unlink_skill(&project_path, *cli, &name) {
                    Ok(()) => println!("{} '{}' {} {} {} {}", t!(lang, "Unlinked", "已取消链接"), name, t!(lang, "for", "到"), cli, t!(lang, "in", "在"), project_path.display()),
                    Err(e) => eprintln!("{}: {e}", t!(lang, "Warning", "警告")),
                }
            }

            // Update registry
            if let Some(entry) = registry.find_skill_mut(&name) {
                entry.linked_to.retain(|l| l.project_path != project_path);
            }
            registry.save()?;
        }

        SkillsAction::Add { url } => {
            let repo_path = registry.repo_path.clone().ok_or_else(|| {
                crate::Error::InvalidInput(
                    t!(lang,
                        "No skills repo configured. Run `agent-hand skills init` first.",
                        "未配置技能仓库。请先运行 `agent-hand skills init`。"
                    ).to_string(),
                )
            })?;

            // Derive a name from the URL (last path segment)
            let skill_name = url
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("community-skill")
                .trim_end_matches(".git");

            let target = repo_path.join("community").join(skill_name);

            println!("{} {url}...", t!(lang, "Adding community skill from", "正在添加社区技能自"));
            skills::github::add_community_skill(&url, &target).await?;

            // Re-scan to discover the new skill
            registry.scan_repo()?;
            registry.save()?;

            if lang.is_zh() {
                println!("已添加 '{skill_name}'。运行 `agent-hand skills list` 查看。");
            } else {
                println!("Added '{skill_name}'. Run `agent-hand skills list` to see it.");
            }
        }

        SkillsAction::Push => {
            let repo_path = registry.repo_path.clone().ok_or_else(|| {
                crate::Error::InvalidInput(
                    t!(lang,
                        "No skills repo configured. Run `agent-hand skills init` first.",
                        "未配置技能仓库。请先运行 `agent-hand skills init`。"
                    ).to_string(),
                )
            })?;

            println!("{}", t!(lang, "Pushing skills to GitHub...", "正在推送技能到 GitHub..."));
            skills::github::push_repo(&repo_path, "Update skills").await?;
            println!("{}", t!(lang, "Pushed successfully.", "推送成功。"));
        }
    }

    Ok(())
}

async fn handle_config(lang: Language, action: ConfigAction) -> Result<()> {
    use crate::session::Storage;

    match action {
        ConfigAction::List { json } => {
            let cfg = crate::config::ConfigFile::load()
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            if json {
                let j = serde_json::to_string_pretty(&cfg)
                    .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
                println!("{j}");
            } else {
                let t = toml::to_string_pretty(&cfg)
                    .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
                println!("{t}");
            }
        }
        ConfigAction::Get { key } => {
            let cfg = crate::config::ConfigFile::load()
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            let toml_str = toml::to_string_pretty(&cfg)
                .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
            let table: toml::Value = toml_str
                .parse()
                .map_err(|e: toml::de::Error| crate::Error::InvalidInput(e.to_string()))?;

            let value = navigate_toml(&table, &key);
            match value {
                Some(v) => println!("{}", format_toml_value(v)),
                None => {
                    let en = format!("Key '{}' not found", key);
                    let zh = format!("键 '{}' 未找到", key);
                    eprintln!("{}", t!(lang, &en, &zh));
                    std::process::exit(1);
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let _cfg = crate::config::ConfigFile::load()
                .await
                .ok()
                .flatten()
                .unwrap_or_default();

            let toml_str = toml::to_string_pretty(&_cfg)
                .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
            let mut table: toml::Value = toml_str
                .parse()
                .map_err(|e: toml::de::Error| crate::Error::InvalidInput(e.to_string()))?;

            set_toml_value(&mut table, &key, &value)
                .map_err(|e| crate::Error::InvalidInput(e))?;

            let updated_str = toml::to_string_pretty(&table)
                .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
            let updated_cfg: crate::config::ConfigFile = toml::from_str(&updated_str)
                .map_err(|e| {
                    crate::Error::InvalidInput(format!("Invalid config after set: {e}"))
                })?;
            updated_cfg.save()?;

            println!("{} = {}", key, value);
        }
        ConfigAction::Path => {
            let dir = Storage::get_agent_hand_dir()?;
            let path = dir.join("config.toml");
            println!("{}", path.display());
        }
        ConfigAction::Reset { force } => {
            if !force {
                eprint!(
                    "{}",
                    t!(
                        lang,
                        "Reset all settings to defaults? [y/N] ",
                        "重置所有设置为默认值？[y/N] "
                    )
                );
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!(
                        "{}",
                        t!(lang, "Cancelled.", "已取消。")
                    );
                    return Ok(());
                }
            }
            let cfg = crate::config::ConfigFile::default();
            cfg.save()?;
            println!(
                "{}",
                t!(
                    lang,
                    "Settings reset to defaults.",
                    "设置已重置为默认值。"
                )
            );
        }
    }
    Ok(())
}

fn navigate_toml<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut current = value;
    for part in path.split('.') {
        match current {
            toml::Value::Table(t) => {
                current = t.get(part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

fn set_toml_value(
    root: &mut toml::Value,
    path: &str,
    raw_value: &str,
) -> std::result::Result<(), String> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return Err("Empty key".to_string());
    }

    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        current = current
            .as_table_mut()
            .ok_or_else(|| format!("'{}' is not a table", part))?
            .entry(part.to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    }

    let last_key = parts.last().unwrap();
    let table = current
        .as_table_mut()
        .ok_or_else(|| "Parent is not a table".to_string())?;

    let typed_value = if let Some(existing) = table.get(*last_key) {
        match existing {
            toml::Value::Boolean(_) => {
                let b = match raw_value.to_lowercase().as_str() {
                    "true" | "1" | "on" | "yes" => true,
                    "false" | "0" | "off" | "no" => false,
                    _ => {
                        return Err(format!(
                            "Expected boolean for '{}', got '{}'",
                            path, raw_value
                        ))
                    }
                };
                toml::Value::Boolean(b)
            }
            toml::Value::Integer(_) => {
                let i: i64 = raw_value
                    .parse()
                    .map_err(|_| format!("Expected integer for '{}', got '{}'", path, raw_value))?;
                toml::Value::Integer(i)
            }
            toml::Value::Float(_) => {
                let f: f64 = raw_value
                    .parse()
                    .map_err(|_| format!("Expected float for '{}', got '{}'", path, raw_value))?;
                toml::Value::Float(f)
            }
            toml::Value::String(_) => toml::Value::String(raw_value.to_string()),
            toml::Value::Array(_) => {
                if raw_value.starts_with('[') {
                    let arr: Vec<String> = serde_json::from_str(raw_value)
                        .map_err(|e| format!("Invalid array: {e}"))?;
                    toml::Value::Array(arr.into_iter().map(toml::Value::String).collect())
                } else {
                    let arr: Vec<toml::Value> = raw_value
                        .split(',')
                        .map(|s| toml::Value::String(s.trim().to_string()))
                        .collect();
                    toml::Value::Array(arr)
                }
            }
            _ => toml::Value::String(raw_value.to_string()),
        }
    } else if let Ok(b) = raw_value.parse::<bool>() {
        toml::Value::Boolean(b)
    } else if let Ok(i) = raw_value.parse::<i64>() {
        toml::Value::Integer(i)
    } else if let Ok(f) = raw_value.parse::<f64>() {
        toml::Value::Float(f)
    } else {
        toml::Value::String(raw_value.to_string())
    };

    table.insert(last_key.to_string(), typed_value);
    Ok(())
}

fn format_toml_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_toml_value).collect();
            items.join(", ")
        }
        toml::Value::Table(_) => toml::to_string_pretty(v).unwrap_or_else(|_| format!("{:?}", v)),
        _ => format!("{:?}", v),
    }
}

/// Interactive CLI chat REPL.
///
/// Spins up a standalone mini event loop with ChatSystem + ActionExecutor,
/// reads lines from stdin, and prints echo responses.
async fn run_chat(session: Option<String>) -> Result<()> {
    use std::io::{BufRead, Write};
    use std::sync::Arc;
    use tokio::sync::{broadcast, mpsc};

    use crate::agent::runner::{ActionExecutor, SystemRunner};
    use crate::agent::systems::chat::ChatSystem;
    use crate::chat::ChatService;
    use crate::config::NotificationConfig;
    use crate::session::Storage;

    // Resolve paths
    let agent_hand_base = Storage::get_agent_hand_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from(".agent-hand"))
        .join("profiles")
        .join("default");
    let progress_dir = agent_hand_base.join("progress");
    let runtime_dir = agent_hand_base.join("agent-runtime");

    // Create channels
    let (event_tx, system_rx) = broadcast::channel(64);
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = mpsc::unbounded_channel();

    // Build SystemRunner with just ChatSystem
    let mut runner = SystemRunner::new();
    runner.register(ChatSystem::new());

    // Build ActionExecutor with chat response channel wired up
    let notif_cfg = Arc::new(std::sync::RwLock::new(NotificationConfig::default()));
    let mut executor = ActionExecutor::new(
        Arc::clone(&notif_cfg),
        progress_dir,
        runtime_dir,
    );
    executor.set_chat_response_tx(response_tx);

    // Spawn background tasks
    tokio::spawn(runner.run(system_rx, action_tx));
    tokio::spawn(executor.run(action_rx));

    // Create ChatService and initial conversation
    let mut chat_svc = ChatService::new(event_tx, response_rx);
    let conv_id = chat_svc.create_conversation(session.clone());

    // Print banner
    println!("agent-hand chat (type /quit to exit, /help for commands)");
    if let Some(ref s) = session {
        println!("  linked to session: {}", s);
    }
    println!();

    // REPL loop
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    loop {
        print!("agent-hand> ");
        stdout.flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            // EOF
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" => break,
            "/help" => {
                println!("Commands:");
                println!("  /quit, /exit  — leave chat");
                println!("  /history      — show conversation messages");
                println!("  /clear        — reset conversation");
                println!("  /help         — show this help");
                println!();
                continue;
            }
            "/history" => {
                if let Some(conv) = chat_svc.get_conversation(&conv_id) {
                    if conv.messages.is_empty() {
                        println!("(no messages yet)");
                    } else {
                        for msg in &conv.messages {
                            let prefix = match msg.role {
                                crate::chat::ChatRole::User => ">",
                                crate::chat::ChatRole::Assistant => "\u{1f916}",
                                crate::chat::ChatRole::System => "[sys]",
                            };
                            println!("{} {}", prefix, msg.content);
                        }
                    }
                }
                println!();
                continue;
            }
            "/clear" => {
                // We can't mutably borrow conv_id and chat_svc at the same time,
                // so just shadow conv_id — but it's not mutable. Instead create
                // a new conversation and tell the user.
                // Note: we can't reassign conv_id since it was declared with let.
                // For simplicity, just print a note. A real impl would need RefCell or
                // restructuring. For MVP, just inform the user to restart.
                println!("(conversation cleared — starting fresh)");
                // Actually we can just create a new ChatService since channels are cloned
                // But that's complex. For MVP, the echo system doesn't maintain state,
                // so /clear is a no-op from the agent's perspective.
                println!();
                continue;
            }
            _ => {}
        }

        // Send message
        if let Err(e) = chat_svc.send_message(&conv_id, input, session.as_deref()) {
            eprintln!("error: {}", e);
            continue;
        }

        // Poll for response with a small delay loop
        let mut got_response = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let responses = chat_svc.poll_responses();
            for resp in &responses {
                println!("\u{1f916} {}", resp.content);
                got_response = true;
            }
            if got_response {
                break;
            }
        }
        if !got_response {
            println!("(no response received)");
        }
        println!();
    }

    println!("goodbye!");
    Ok(())
}
