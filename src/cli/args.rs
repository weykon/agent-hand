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

    /// Authenticate to unlock premium features
    Login,

    /// Remove stored authentication credentials
    Logout,

    /// Show account status and license info
    Account {
        /// Refresh features from server before displaying
        #[arg(long)]
        refresh: bool,
    },

    /// List and manage registered devices
    Devices {
        /// Remove a device by ID prefix
        #[arg(long)]
        remove: Option<String>,
    },

    /// Share a session remotely via tmate (Premium)
    Share {
        /// Session ID or title
        id: String,

        /// Permission level: "ro" (read-only) or "rw" (read-write)
        #[arg(long, default_value = "ro")]
        permission: String,

        /// Auto-expire after N minutes
        #[arg(long)]
        expire: Option<u64>,
    },

    /// Stop sharing a session
    Unshare {
        /// Session ID or title
        id: String,
    },

    /// Join a shared session via relay URL
    Join {
        /// Share URL (e.g. https://relay.asymptai.com/share/ROOM_ID?token=TOKEN)
        url: String,
    },

    /// Interactive chat with the agent system
    Chat {
        /// Link chat to a specific session by ID or title
        #[arg(short, long)]
        session: Option<String>,
    },

    /// Interact with the canvas workflow editor (requires running TUI)
    Canvas {
        #[command(subcommand)]
        action: CanvasAction,
    },

    /// Manage skills library (Pro)
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },

    /// View and modify configuration settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Internal: PTY viewer bridge (runs inside tmux pane, bridges WebSocket ↔ stdio)
    #[command(name = "pty-viewer")]
    PtyViewer {
        /// Relay server URL
        #[arg(long)]
        relay_url: String,

        /// Room ID to join
        #[arg(long)]
        room_id: String,

        /// Viewer authentication token
        #[arg(long)]
        token: String,

        /// Tmux session name (for resize forwarding)
        #[arg(long)]
        session_name: String,

        /// Optional user account token for RW access
        #[arg(long)]
        user_token: Option<String>,

        /// Optional display name
        #[arg(long)]
        display_name: Option<String>,
    },

    /// Internal: Viewer info popup (shown via tmux display-popup)
    #[command(name = "viewer-info")]
    ViewerInfo {
        /// Room ID to show info for
        #[arg(long)]
        room_id: String,
    },
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

#[derive(Subcommand, Debug)]
pub enum CanvasAction {
    /// Add a node to the canvas
    AddNode {
        /// Unique node ID
        #[arg(long)]
        id: String,

        /// Display label
        #[arg(long)]
        label: String,

        /// Node kind: start, end, process, decision, note
        #[arg(long, default_value = "process")]
        kind: String,

        /// Position as "col,row" (auto-placed if omitted)
        #[arg(long)]
        pos: Option<String>,
    },

    /// Remove a node from the canvas
    RemoveNode {
        /// Node ID to remove
        #[arg(long)]
        id: String,
    },

    /// Add an edge between two nodes
    AddEdge {
        /// Source node ID
        #[arg(long)]
        from: String,

        /// Target node ID
        #[arg(long)]
        to: String,

        /// Edge label
        #[arg(long)]
        label: Option<String>,
    },

    /// Remove an edge between two nodes
    RemoveEdge {
        /// Source node ID
        #[arg(long)]
        from: String,

        /// Target node ID
        #[arg(long)]
        to: String,
    },

    /// Trigger auto-layout
    Layout {
        /// Direction: top-down or left-right
        #[arg(long, default_value = "top-down")]
        direction: String,
    },

    /// Query canvas state
    Query {
        /// What to query: nodes, edges, state, selected
        what: String,
    },

    /// Send a batch of operations from a JSON file
    Batch {
        /// Path to JSON file containing an array of CanvasOp objects
        #[arg(long)]
        file: String,
    },

    /// Send a raw JSON CanvasOp string
    Raw {
        /// JSON string
        json: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// List all configuration settings
    List {
        /// Output as JSON instead of TOML
        #[arg(long)]
        json: bool,
    },
    /// Get a specific setting value
    Get {
        /// Setting key (dot notation, e.g. notification.volume)
        key: String,
    },
    /// Set a specific setting value
    Set {
        /// Setting key (dot notation, e.g. notification.volume)
        key: String,
        /// New value
        value: String,
    },
    /// Show configuration file path
    Path,
    /// Reset all settings to defaults
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SkillsAction {
    /// Initialize skills repository on GitHub
    Init {
        #[arg(long, default_value = "agent-skills")]
        repo: String,
    },
    /// Pull latest changes from GitHub
    Sync,
    /// List all skills
    List {
        #[arg(long)]
        json: bool,
    },
    /// Link a skill to a project or group
    Link {
        name: String,
        #[arg(long)]
        group: Option<String>,
    },
    /// Unlink a skill from a project
    Unlink { name: String },
    /// Add a community skill from GitHub URL
    Add { url: String },
    /// Push local changes to GitHub
    Push,
}
