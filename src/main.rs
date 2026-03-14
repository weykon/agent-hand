use agent_hand::cli::{run_cli, Args};
use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

/// Ensure common system directories are in PATH.
///
/// WSL non-login shells (e.g. `wsl -e agent-hand` or WSL interop from PowerShell)
/// may have a minimal PATH that excludes /usr/bin where tmux lives.
/// See: <https://github.com/microsoft/WSL/issues/3627>
fn ensure_system_paths() {
    const REQUIRED: &[&str] = &[
        "/usr/local/bin",
        "/usr/bin",
        "/usr/local/sbin",
        "/usr/sbin",
        "/sbin",
        "/bin",
    ];

    let current = std::env::var("PATH").unwrap_or_default();
    let parts: std::collections::HashSet<&str> = current.split(':').collect();

    let missing: Vec<&&str> = REQUIRED.iter().filter(|d| !parts.contains(**d)).collect();
    if !missing.is_empty() {
        let additions = missing.iter().map(|d| **d).collect::<Vec<_>>().join(":");
        let new_path = if current.is_empty() {
            additions
        } else {
            format!("{}:{}", current, additions)
        };
        std::env::set_var("PATH", new_path);
    }
}

#[tokio::main]
async fn main() {
    // Fix PATH for WSL non-login shells before anything else
    ensure_system_paths();

    // Initialize logging — write to file to avoid polluting the TUI
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agent-hand");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("agent-hand.log");

    if let Ok(log_file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(log_file))
            .init();
    } else {
        // Fallback: stderr (will leak through TUI, but at least we have logging)
        fmt().with_env_filter(filter).with_target(false).init();
    }

    // Parse CLI args
    let args = Args::parse();

    // Run CLI or TUI
    if let Err(e) = run_cli(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
