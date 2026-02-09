use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;

/// Result of a system-wide ptmx scan.
#[derive(Debug, Clone, Default)]
pub struct PtmxReport {
    /// Session ID → number of /dev/ptmx FDs held by that session's process tree.
    pub per_session: HashMap<String, u32>,
    /// Total /dev/ptmx FDs open system-wide.
    pub system_total: u32,
    /// System PTY limit.
    pub system_max: u32,
}

/// Shared state for PTY monitoring, updated by background task.
#[derive(Debug, Clone, Default)]
pub struct PtmxState {
    /// Session ID → PTY count
    pub per_session: HashMap<String, u32>,
    /// System-wide total
    pub system_total: u32,
    /// System max limit
    pub system_max: u32,
    /// Last scan timestamp
    pub last_scan: Option<Instant>,
    /// Whether a scan is currently running
    pub is_scanning: bool,
}

/// Shared handle to PTY state
pub type SharedPtmxState = Arc<RwLock<PtmxState>>;

/// Detect the system PTY limit.
///
/// macOS: `sysctl -n kern.tty.ptmx_max`
/// Linux: `/proc/sys/kernel/pty/max`
/// Fallback: 256
pub async fn get_ptmx_max() -> u32 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("sysctl")
            .args(["-n", "kern.tty.ptmx_max"])
            .output()
            .await
        {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(n) = s.trim().parse::<u32>() {
                    return n;
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = tokio::fs::read_to_string("/proc/sys/kernel/pty/max").await {
            if let Ok(n) = contents.trim().parse::<u32>() {
                return n;
            }
        }
    }

    256
}

/// Build a PID → ptmx-FD-count map by running `lsof /dev/ptmx` once.
async fn lsof_ptmx_counts() -> HashMap<u32, u32> {
    let mut map: HashMap<u32, u32> = HashMap::new();

    let Ok(out) = Command::new("lsof")
        .arg("/dev/ptmx")
        .output()
        .await
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    // lsof output: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
    // Skip header line, parse PID from column 2.
    for line in stdout.lines().skip(1) {
        let mut cols = line.split_whitespace();
        if let Some(pid_str) = cols.nth(1) {
            if let Ok(pid) = pid_str.parse::<u32>() {
                *map.entry(pid).or_insert(0) += 1;
            }
        }
    }

    map
}

/// Collect all descendant PIDs of `root_pid` (inclusive) via `pgrep -P`.
async fn collect_process_tree(root_pid: u32) -> Vec<u32> {
    let mut result = vec![root_pid];
    let mut queue = vec![root_pid];

    while let Some(pid) = queue.pop() {
        let Ok(out) = Command::new("pgrep")
            .args(["-P", &pid.to_string()])
            .output()
            .await
        else {
            continue;
        };
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if let Ok(child) = line.trim().parse::<u32>() {
                result.push(child);
                queue.push(child);
            }
        }
    }

    result
}

/// Get pane PIDs for all sessions on the agent-deck tmux server.
/// Returns `(session_name, pane_pid)` pairs.
async fn get_tmux_pane_pids() -> Vec<(String, u32)> {
    let Ok(out) = Command::new("tmux")
        .args([
            "-L", super::manager::TMUX_SERVER_NAME,
            "list-panes", "-a",
            "-F", "#{session_name} #{pane_pid}",
        ])
        .output()
        .await
    else {
        return Vec::new();
    };

    if !out.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let pid = parts.next()?.parse::<u32>().ok()?;
            if name.starts_with(super::SESSION_PREFIX) {
                Some((name.to_string(), pid))
            } else {
                None
            }
        })
        .collect()
}

/// Scan the system for ptmx usage and attribute FDs to known sessions.
///
/// Runs `lsof /dev/ptmx` once, then for each tmux session walks the
/// process tree to sum up ptmx FDs belonging to that session.
pub async fn scan_ptmx_usage(system_max: u32) -> PtmxReport {
    let (fd_counts, pane_pids) =
        tokio::join!(lsof_ptmx_counts(), get_tmux_pane_pids());

    let system_total: u32 = fd_counts.values().sum();

    let mut per_session: HashMap<String, u32> = HashMap::new();

    for (session_name, pane_pid) in &pane_pids {
        let tree = collect_process_tree(*pane_pid).await;
        let count: u32 = tree
            .iter()
            .filter_map(|pid| fd_counts.get(pid))
            .sum();
        if count > 0 {
            // Strip the session prefix to get the instance ID.
            let id = session_name
                .strip_prefix(super::SESSION_PREFIX)
                .unwrap_or(session_name)
                .to_string();
            per_session.insert(id, count);
        }
    }

    PtmxReport {
        per_session,
        system_total,
        system_max,
    }
}

/// Spawn a background task that periodically scans PTY usage.
///
/// The task runs immediately upon spawn, then every 30 minutes.
/// It updates the shared state which can be read by the UI thread.
pub fn spawn_ptmx_monitor(
    system_max: u32,
    state: SharedPtmxState,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Perform initial scan immediately
        perform_scan(&state, system_max).await;

        // Then scan every 30 minutes
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));

        loop {
            interval.tick().await;
            perform_scan(&state, system_max).await;
        }
    })
}

/// Perform a single PTY scan and update the shared state.
async fn perform_scan(state: &SharedPtmxState, system_max: u32) {
    // Mark as scanning
    {
        let mut guard = state.write().await;
        guard.is_scanning = true;
    }

    // Perform the actual scan
    let report = scan_ptmx_usage(system_max).await;

    // Update state
    {
        let mut guard = state.write().await;
        guard.per_session = report.per_session;
        guard.system_total = report.system_total;
        guard.system_max = report.system_max;
        guard.last_scan = Some(Instant::now());
        guard.is_scanning = false;
    }
}
