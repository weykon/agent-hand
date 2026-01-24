use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command as TokioCommand;

use crate::cli::{Args, Command, ProfileAction, SessionAction};
use crate::error::Result;
use crate::session::{Instance, Storage, DEFAULT_PROFILE};
use crate::tmux::TmuxManager;
use tracing::warn;

pub async fn run_cli(args: Args) -> Result<()> {
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
    if cfg.as_ref().is_some_and(|c| c.claude_user_prompt_logging()) {
        if let Err(err) = crate::claude::ensure_user_prompt_hook().await {
            warn!("failed to ensure Claude hook: {err}");
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
        }) => handle_add(profile, path, title, group, cmd).await,

        Some(Command::List { json, all }) => handle_list(profile, json, all).await,

        Some(Command::Remove { identifier }) => handle_remove(profile, &identifier).await,

        Some(Command::Status {
            verbose,
            quiet,
            json,
        }) => handle_status(profile, verbose, quiet, json).await,

        Some(Command::Statusline) => handle_statusline(profile).await,

        Some(Command::Session { action }) => handle_session(profile, action).await,

        Some(Command::Profile { action }) => handle_profile(action).await,

        Some(Command::Upgrade { prefix, version }) => handle_upgrade(prefix, version).await,

        Some(Command::Switch) => crate::ui::run_switcher(profile).await,

        Some(Command::Jump) => handle_jump(profile).await,

        Some(Command::Version) => {
            println!("agent-hand v{}", crate::VERSION);
            Ok(())
        }

        None => {
            // Check tmux availability before launching TUI
            if !crate::tmux::TmuxManager::is_available()
                .await
                .unwrap_or(false)
            {
                eprintln!("Error: tmux is not installed or not in PATH");
                eprintln!();
                eprintln!("agent-hand requires tmux to manage terminal sessions.");
                eprintln!();
                eprintln!("Install tmux:");
                eprintln!("  macOS:        brew install tmux");
                eprintln!("  Ubuntu/Debian: sudo apt install tmux");
                eprintln!("  Fedora:       sudo dnf install tmux");
                eprintln!("  Arch:         sudo pacman -S tmux");
                eprintln!();
                eprintln!("Or visit: https://github.com/tmux/tmux/wiki/Installing");
                return Err(crate::Error::tmux("tmux is not installed"));
            }

            // Launch TUI
            let mut app = crate::ui::App::new(profile).await?;
            app.run().await
        }
    }
}

async fn handle_upgrade(prefix: Option<String>, version: Option<String>) -> Result<()> {
    const REPO: &str = "weykon/agent-hand";
    const BIN_NAME: &str = "agent-hand";

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let os = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => return Err(crate::Error::InvalidInput(format!("Unsupported OS: {os}"))),
    };

    let arch = match arch {
        "x86_64" | "amd64" => "x86_64",
        "aarch64" | "arm64" => "aarch64",
        _ => {
            return Err(crate::Error::InvalidInput(format!(
                "Unsupported arch: {arch}"
            )))
        }
    };

    let target = format!("{arch}-{os}");
    let asset = format!("{BIN_NAME}-{target}.tar.gz");

    let version = version.unwrap_or_else(|| "latest".to_string());
    let url_base = format!("https://github.com/{REPO}/releases");
    let url = if version == "latest" {
        format!("{url_base}/latest/download/{asset}")
    } else {
        format!("{url_base}/download/{version}/{asset}")
    };

    let prefix = if let Some(p) = prefix {
        PathBuf::from(p)
    } else if is_dir_writable(Path::new("/usr/local/bin")) {
        PathBuf::from("/usr/local/bin")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/bin")
    };

    std::fs::create_dir_all(&prefix)?;

    let tmpdir = std::env::temp_dir().join(format!("agent-hand-upgrade-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmpdir)?;

    let tar_path = tmpdir.join(&asset);

    eprintln!("Downloading {url}");
    let status = TokioCommand::new("curl")
        .args(["-fsSL", &url, "-o"])
        .arg(&tar_path)
        .status()
        .await?;
    if !status.success() {
        return Err(crate::Error::InvalidInput(
            "Failed to download release asset".to_string(),
        ));
    }

    let status = TokioCommand::new("tar")
        .args(["-xzf"])
        .arg(&tar_path)
        .args(["-C"])
        .arg(&tmpdir)
        .status()
        .await?;
    if !status.success() {
        return Err(crate::Error::InvalidInput(
            "Failed to extract release archive".to_string(),
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
        eprintln!("Installed {BIN_NAME} to {}", dest.display());
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

    eprintln!("Installed {BIN_NAME} to {}", dest.display());
    let _ = std::fs::remove_dir_all(&tmpdir);
    Ok(())
}

fn is_dir_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let test = dir.join(format!(".agent-hand-write-test-{}", uuid::Uuid::new_v4()));
    let ok = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&test)
        .is_ok();
    let _ = std::fs::remove_file(&test);
    ok
}

async fn handle_add(
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
        eprintln!("Created directory: {}", project_path.display());
    }

    let project_path = project_path.canonicalize()?;

    // Verify path exists and is directory
    if !project_path.is_dir() {
        return Err(crate::Error::InvalidInput(format!(
            "Path is not a directory: {}",
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
    let (mut instances, tree) = storage.load().await?;

    // Check for duplicates
    for inst in &instances {
        if inst.project_path == project_path {
            println!("✓ Session already exists: {} ({})", inst.title, inst.id);
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
    storage.save(&instances, &tree).await?;

    println!("✓ Added session: {}", title);
    println!("  Profile: {}", profile);
    println!("  Path:    {}", project_path.display());
    println!("  Group:   {}", instance.group_path);
    println!("  ID:      {}", instance.id);

    Ok(())
}

async fn handle_list(profile: &str, json: bool, all: bool) -> Result<()> {
    if all {
        let profiles = Storage::list_profiles().await?;
        for prof in profiles {
            println!("\n=== Profile: {} ===", prof);
            list_profile(&prof, json).await?;
        }
        return Ok(());
    }

    list_profile(profile, json).await
}

async fn list_profile(profile: &str, json: bool) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, _) = storage.load().await?;

    if instances.is_empty() {
        if !json {
            println!("No sessions found in profile '{}'.", profile);
        }
        return Ok(());
    }

    if json {
        let json_str = serde_json::to_string_pretty(&instances)?;
        println!("{}", json_str);
    } else {
        println!("Profile: {}\n", profile);
        println!("{:<20} {:<15} {:<40} {}", "TITLE", "GROUP", "PATH", "ID");
        println!("{}", "-".repeat(90));

        for inst in &instances {
            // <-- Added &
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

        println!("\nTotal: {} sessions", instances.len());
    }

    Ok(())
}

async fn handle_remove(profile: &str, identifier: &str) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (instances, tree) = storage.load().await?;

    let (to_remove, to_keep): (Vec<_>, Vec<_>) = instances.into_iter().partition(|inst| {
        inst.id == identifier || inst.id.starts_with(identifier) || inst.title == identifier
    });

    if to_remove.is_empty() {
        return Err(crate::Error::SessionNotFound(identifier.to_string()));
    }

    let removed = &to_remove[0];
    let title = removed.title.clone();

    // Kill tmux session if exists
    let manager = TmuxManager::new();
    let tmux_name = removed.tmux_name();
    if manager.session_exists(&tmux_name).unwrap_or(false) {
        if let Err(e) = manager.kill_session(&tmux_name).await {
            eprintln!("Warning: failed to kill tmux session: {}", e);
        }
    }

    // Save
    storage.save(&to_keep, &tree).await?;

    println!("✓ Removed session: {} (from profile '{}')", title, profile);
    Ok(())
}

async fn handle_status(profile: &str, verbose: bool, quiet: bool, json: bool) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (mut instances, _) = storage.load().await?;

    if instances.is_empty() {
        if json {
            println!(r#"{{"waiting": 0, "running": 0, "idle": 0, "error": 0, "total": 0}}"#);
        } else if !quiet {
            println!("No sessions in profile '{}'.", profile);
        }
        return Ok(());
    }

    // Update statuses
    let manager = Arc::new(TmuxManager::new());
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
        print_status_verbose(&instances);
    } else {
        println!(
            "{} waiting • {} running • {} idle",
            counts.waiting, counts.running, counts.idle
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
    let (mut instances, tree) = storage.load().await?;

    if instances.is_empty() {
        println!("AH");
        return Ok(());
    }

    let manager = Arc::new(TmuxManager::new());
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
        storage.save(&instances, &tree).await?;
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
    let (mut instances, _tree) = storage.load().await?;

    if instances.is_empty() {
        // Use tmux display-message instead of eprintln so user sees it in tmux
        let _ = TokioCommand::new("tmux")
            .args(["-L", "agentdeck_rs", "display-message", "AH: no sessions"])
            .status()
            .await;
        return Ok(());
    }

    let manager = Arc::new(TmuxManager::new());
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

async fn handle_session(profile: &str, action: SessionAction) -> Result<()> {
    let storage = Storage::new(profile).await?;
    let (mut instances, tree) = storage.load().await?;
    let manager = Arc::new(TmuxManager::new());

    match action {
        SessionAction::Start { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone(); // Clone before operations
            inst.init_tmux(manager.clone());
            inst.start().await?;
            storage.save(&instances, &tree).await?;
            println!("✓ Started session: {}", title);
        }

        SessionAction::Stop { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone();
            inst.init_tmux(manager.clone());
            inst.stop().await?;
            storage.save(&instances, &tree).await?;
            println!("✓ Stopped session: {}", title);
        }

        SessionAction::Restart { id } => {
            let inst = find_session(&mut instances, &id)?;
            let title = inst.title.clone();
            inst.init_tmux(manager.clone());
            inst.stop().await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            inst.start().await?;
            storage.save(&instances, &tree).await?;
            println!("✓ Restarted session: {}", title);
        }

        SessionAction::Attach { id } => {
            let inst = find_session(&mut instances, &id)?;
            inst.init_tmux(manager.clone());
            inst.attach().await?;
            storage.save(&instances, &tree).await?;
        }

        SessionAction::Show { id } => {
            let inst = if let Some(id_str) = &id {
                find_session(&mut instances, id_str)?
            } else {
                // Auto-detect from current tmux session
                return Err(crate::Error::InvalidInput(
                    "Auto-detection not yet implemented".to_string(),
                ));
            };

            println!("Session: {}", inst.title);
            println!("  ID:      {}", inst.id);
            println!("  Path:    {}", inst.project_path.display());
            println!("  Group:   {}", inst.group_path);
            println!("  Status:  {:?}", inst.status);
            println!("  Created: {}", inst.created_at);
        }
    }

    Ok(())
}

async fn handle_profile(action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            let profiles = Storage::list_profiles().await?;
            println!("Profiles:");
            for prof in profiles {
                println!("  {}", prof);
            }
        }

        ProfileAction::Create { name } => {
            Storage::create_profile(&name).await?;
            println!("✓ Created profile: {}", name);
        }

        ProfileAction::Delete { name } => {
            Storage::delete_profile(&name).await?;
            println!("✓ Deleted profile: {}", name);
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
        s[..max].to_string()
    } else {
        format!("{}...", &s[..max - 3])
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

fn print_status_verbose(instances: &[Instance]) {
    let symbols = [
        (crate::session::Status::Waiting, "◐", "WAITING"),
        (crate::session::Status::Running, "●", "RUNNING"),
        (crate::session::Status::Idle, "○", "IDLE"),
        (crate::session::Status::Error, "✕", "ERROR"),
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
