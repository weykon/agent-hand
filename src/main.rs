use agent_hand::cli::{run_cli, Args};
use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt().with_env_filter(filter).with_target(false).init();

    // Parse CLI args
    let args = Args::parse();

    // Run CLI or TUI
    if let Err(e) = run_cli(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
