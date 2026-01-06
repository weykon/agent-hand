use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command as TokioCommand;

use crate::cli::{Args, Command, McpSubAction, PoolAction, ProfileAction, SessionAction};
use crate::error::Result;
use crate::session::{Instance, Storage, DEFAULT_PROFILE};
use crate::tmux::{TmuxManager, Tool};

pub async fn run_cli(args: Args) -> Result<()> {
    // Backward-compat: allow legacy env var name.
    let legacy_profile = std::env::var("AGENTDECK_PROFILE").ok();
    let profile = args
        .profile
        .as_deref()
        .or(legacy_profile.as_deref())
        .unwrap_or(DEFAULT_PROFILE);

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

        Some(Command::Session { action }) => handle_session(profile, action).await,

        Some(Command::Profile { action }) => handle_profile(action).await,

        Some(Command::Mcp { action }) => handle_mcp(action).await,

        Some(Command::Upgrade { prefix, version }) => handle_upgrade(prefix, version).await,

        Some(Command::Version) => {
            println!("agent-hand v{}", crate::VERSION);
            Ok(())
        }

        None => {
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
        instance.tool = Tool::from_command(&command);
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

async fn handle_mcp(action: McpSubAction) -> Result<()> {
    use crate::mcp::MCPPool;

    match action {
        McpSubAction::Pool { action } => match action {
            PoolAction::Start { name } => {
                MCPPool::start(&name).await?;
                println!("✓ MCP pool started: {name}");
                println!("  Socket: {}", MCPPool::socket_path(&name)?.display());
                Ok(())
            }
            PoolAction::Serve { name } => MCPPool::serve(&name).await,
            PoolAction::Stop { name } => {
                MCPPool::stop(&name).await?;
                println!("✓ MCP pool stopped: {name}");
                Ok(())
            }
            PoolAction::Status => {
                let names = MCPPool::list_available().await?;
                for n in names {
                    let running = MCPPool::is_running(&n).await;
                    println!("{} {}", if running { "●" } else { "○" }, n);
                }
                Ok(())
            }
            PoolAction::List => {
                let names = MCPPool::list_available().await?;
                for n in names {
                    println!("{n}");
                }
                Ok(())
            }
        },
    }
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
