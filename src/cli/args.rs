use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "agent-hand")]
#[command(version, about = "Terminal session manager for AI coding agents", long_about = None)]
pub struct Args {
    /// Profile to use
    #[arg(short, long, global = true, env = "AGENTHAND_PROFILE")]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Add a new session
    Add {
        /// Project directory path
        path: Option<String>,

        /// Session title
        #[arg(short, long)]
        title: Option<String>,

        /// Group path
        #[arg(short, long)]
        group: Option<String>,

        /// Command to run
        #[arg(short, long)]
        cmd: Option<String>,
    },

    /// List all sessions
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// List sessions from all profiles
        #[arg(long)]
        all: bool,
    },

    /// Remove a session
    Remove {
        /// Session ID or title
        identifier: String,
    },

    /// Show session status
    Status {
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Quiet mode (just count)
        #[arg(short, long)]
        quiet: bool,

        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// Print a compact one-line status for tmux status-left
    Statusline,

    /// Session management commands
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Upgrade agent-hand from GitHub Releases
    Upgrade {
        /// Install directory (default: /usr/local/bin if writable, else ~/.local/bin)
        #[arg(long)]
        prefix: Option<String>,

        /// Release tag (default: latest)
        #[arg(long)]
        version: Option<String>,
    },

    /// Popup session switcher (for tmux Ctrl+G)
    Switch,

    /// Jump to priority session (for tmux Ctrl+N)
    Jump,

    /// Show version
    Version,
}

#[derive(Subcommand, Debug)]
pub enum SessionAction {
    /// Start a session
    Start { id: String },

    /// Stop a session
    Stop { id: String },

    /// Restart a session
    Restart { id: String },

    /// Attach to a session
    Attach { id: String },

    /// Show session details
    Show { id: Option<String> },
}

#[derive(Subcommand, Debug)]
pub enum ProfileAction {
    /// List all profiles
    List,

    /// Create a new profile
    Create { name: String },

    /// Delete a profile
    Delete { name: String },
}
