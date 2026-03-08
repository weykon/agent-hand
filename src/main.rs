use agent_hand::cli::{run_cli, Args};
use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
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
