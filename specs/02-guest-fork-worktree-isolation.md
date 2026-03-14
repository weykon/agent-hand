# Guest Fork & Worktree Isolation — SPEC

## 1. Overview

### Problem

In the current collaboration model, a viewer watches the host work in real-time but has no way to **branch off** and start their own parallel work. When a viewer sees the host tackle a problem and thinks "I know how to fix that" or "I want to try a different approach," they must:

1. Manually clone the repo on their own machine
2. Check out the right branch/commit
3. Set up their own AI coding agent
4. Lose all context from the live session

This friction kills the spontaneous, pair-programming-style collaboration that makes live sharing valuable.

### Solution

**Guest Fork**: A viewer watching a host session can press a keybinding to "fork" the current context into their own isolated workspace. The system:

1. Creates a **git worktree** from the host's current branch/commit
2. Opens a **new tmux session** in that worktree
3. Optionally **launches an AI agent** (Claude Code, Cursor, etc.) in the fork
4. **Registers the fork** as a child session in the host's Agent Hand instance
5. **Isolates** the guest: they can write to their worktree but cannot touch the host's working directory

The host maintains full visibility over guest forks — seeing who forked, from what point, and what they're working on. Forks can be cleaned up manually or expire via configurable TTL.

### Design Principles

1. **Isolation by default**: Guest forks use git worktree, which provides filesystem-level separation. A guest can never corrupt the host's working tree.
2. **Zero-config for guests**: The fork operation should require no setup — one keybinding, automatic worktree creation.
3. **Host sovereignty**: The host controls whether fork is allowed, sees all active forks, and can revoke/clean up at any time.
4. **Context continuity**: The fork captures the exact branch+commit the host was on, preserving the "I saw this and want to try something" moment.
5. **Lightweight lifecycle**: Forks are temporary by nature. Sensible defaults for auto-cleanup prevent worktree sprawl.

---

## 2. User Stories

### US-1: Guest forks from viewer mode
> As a **viewer** watching a host debug a failing test, I want to press `Ctrl+F` to fork the current session into my own worktree, so I can try a different fix approach without disrupting the host.

### US-2: Host sees active forks
> As a **host**, I want to see a list of guest forks spawned from my session (who forked, when, from which commit), so I can track parallel work happening on my project.

### US-3: Guest launches AI agent in fork
> As a **guest** who just forked, I want my fork to automatically launch Claude Code (or my preferred agent) in the worktree, so I can start working immediately without manual setup.

### US-4: Host controls fork permission
> As a **host**, I want to configure whether viewers can fork my session (allow-all, require-approval, deny), so I maintain control over who creates worktrees on my machine.

### US-5: Fork cleanup
> As a **host**, I want guest forks to auto-expire after 24 hours of inactivity, so abandoned worktrees don't accumulate on my filesystem.

### US-6: Guest contributes back
> As a **guest** who found a fix in my fork, I want to create a branch and push it so the host can review and merge my changes. The fork's worktree is already on a separate branch, so this is a natural git operation.

### US-7: Host revokes a fork
> As a **host**, I want to revoke an active fork (killing the guest's session and cleaning up the worktree), in case a guest is doing something I don't want on my machine.

### US-8: Fork with specific context
> As a **viewer**, when I fork, I want the fork to capture not just the branch but the **specific commit** the host was on at that moment, so I'm working from a known-good state even if the host commits further changes.

---

## 3. Architecture

### 3.1 Approach Evaluation

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| **A. Git worktree** | Native git isolation; shared object store (fast); separate branch per fork; easy cleanup via `git worktree remove` | Requires repo on host machine; worktrees share `.git`; limited to git repos | ✅ **Selected** |
| **B. Full clone** | Complete isolation; works with any VCS | Slow for large repos; doubles disk usage; no shared object store | ❌ Too heavy |
| **C. Docker container** | Strongest isolation; reproducible env | Requires Docker; complex setup; overhead; not TUI-native | ❌ Over-engineered |
| **D. Filesystem overlay (overlayfs)** | Copy-on-write; lightweight | Linux-only; root required; fragile with git | ❌ Platform-limited |

### 3.2 System Architecture

```
HOST MACHINE
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  ┌─────────────────────┐           ┌──────────────────────────────────┐ │
│  │   Host Session       │           │   Guest Fork Registry            │ │
│  │   (tmux: project_a1) │           │                                  │ │
│  │                       │  creates  │   fork_id: "gf_abc123"          │ │
│  │   Working Dir:        │ ───────→  │   guest_id: "viewer@email"      │ │
│  │   ~/projects/myapp    │           │   source_commit: "a1b2c3d"      │ │
│  │   Branch: main        │           │   source_branch: "main"         │ │
│  │                       │           │   worktree_path: ~/.agent-hand/ │ │
│  │   Viewers:            │           │     worktrees/gf_abc123/        │ │
│  │     viewer@email (👁) │           │   tmux_session: "fork_gf_abc1"  │ │
│  │                       │           │   created_at: 2026-03-06T...    │ │
│  └─────────────────────┘           │   expires_at: 2026-03-07T...    │ │
│                                     │   status: Active                 │ │
│                                     └──────────────────────────────────┘ │
│                                                                         │
│  ┌──────────────────────────────────────────────────┐                   │
│  │   Git Repository: ~/projects/myapp/.git           │                   │
│  │                                                    │                   │
│  │   Worktrees:                                       │                   │
│  │     ~/projects/myapp  (main)        ← host         │                   │
│  │     ~/.agent-hand/worktrees/gf_abc123/             │                   │
│  │       (branch: fork/viewer@email/2026-03-06-1)     │                   │
│  │       ← guest (isolated filesystem)                │                   │
│  └──────────────────────────────────────────────────┘                   │
│                                                                         │
│  ┌──────────────────────────────────────────┐                           │
│  │   Guest Fork tmux Session                 │                           │
│  │   (tmux: fork_gf_abc1)                    │                           │
│  │                                            │                           │
│  │   CWD: ~/.agent-hand/worktrees/gf_abc123/ │                           │
│  │   Tool: Claude Code (or guest's choice)    │                           │
│  │   Parent: project_a1 (host session)        │                           │
│  └──────────────────────────────────────────┘                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Component Interactions

```
Viewer UI (viewer mode)              Host App
─────────────────────                ────────
      │                                 │
      │  Ctrl+F (fork request)          │
      │ ──────────────────────────────→ │
      │                                 │
      │         (if policy=approve)     │
      │  ← ─ ─ "Fork requested by      │
      │        viewer@email. Allow?"    │
      │                                 │
      │         (host approves or       │
      │          policy=allow-all)      │
      │                                 │
      │                          ┌──────┴───────┐
      │                          │ Fork Engine  │
      │                          │              │
      │                          │ 1. Resolve   │
      │                          │    HEAD      │
      │                          │ 2. git       │
      │                          │    worktree  │
      │                          │    add       │
      │                          │ 3. Create    │
      │                          │    tmux sess │
      │                          │ 4. Register  │
      │                          │    fork      │
      │                          │ 5. Launch    │
      │                          │    agent     │
      │                          └──────┬───────┘
      │                                 │
      │  ForkCreated { fork_id,         │
      │    worktree_path, branch,       │
      │    tmux_session }               │
      │ ←────────────────────────────── │
      │                                 │
      │  (viewer switches to fork       │
      │   session in tmux)              │
```

### 3.4 Worktree Placement

Worktrees are placed under Agent Hand's data directory, not inside the project:

```
~/.agent-hand/
  worktrees/
    gf_abc123/          ← fork worktree (actual code files here)
    gf_def456/          ← another fork
  profiles/
    default/
      sessions.json     ← fork sessions registered here
```

**Rationale**: Placing worktrees outside the project directory prevents them from appearing in the host's IDE, file watchers, or `git status`. The host's workspace stays clean.

---

## 4. Protocol / API

### 4.1 Data Structures

```rust
/// Represents a guest fork of a host session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestFork {
    /// Unique fork identifier (12-char, prefixed "gf_")
    pub fork_id: String,

    /// The host session this was forked from
    pub source_session_id: String,

    /// Git commit SHA at the moment of fork
    pub source_commit: String,

    /// Branch name at the moment of fork
    pub source_branch: String,

    /// Guest identity (email or display name from viewer auth)
    pub guest_id: String,

    /// Path to the created worktree
    pub worktree_path: PathBuf,

    /// Branch created for this fork
    pub fork_branch: String,

    /// The tmux session name for this fork
    pub tmux_session: String,

    /// Session Instance ID (registered in sessions.json)
    pub instance_id: String,

    /// AI tool to launch in the fork (None = shell only)
    pub tool: Option<Tool>,

    /// Fork lifecycle
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ForkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForkStatus {
    /// Fork request sent, awaiting host approval
    Pending,
    /// Worktree being created
    Creating,
    /// Fork is active and usable
    Active,
    /// Guest detached but fork preserved
    Detached,
    /// Fork expired or revoked, cleanup pending
    Expired,
    /// Worktree removed, session deleted
    Cleaned,
}

/// Host-side fork policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForkPolicy {
    /// Any viewer can fork without approval
    AllowAll,
    /// Each fork requires host approval via dialog
    RequireApproval,
    /// Forking is disabled
    Deny,
    /// Only viewers with read-write permission can fork
    ReadWriteOnly,
}
```

### 4.2 Control Messages (extend existing protocol)

```rust
/// New variants for ControlMessage enum
enum ControlMessage {
    // ... existing variants ...

    /// Viewer requests to fork the current session
    ForkRequest {
        viewer_id: String,
        /// Optional: guest's preferred AI tool
        preferred_tool: Option<String>,
    },

    /// Host responds to fork request
    ForkResponse {
        viewer_id: String,
        approved: bool,
        /// If approved, contains fork details
        fork_info: Option<ForkCreatedInfo>,
        /// If denied, optional reason
        reason: Option<String>,
    },

    /// Broadcast: a fork was created (visible to all viewers)
    ForkCreated {
        fork_id: String,
        guest_display_name: String,
        source_commit: String,
        fork_branch: String,
    },

    /// Host revokes a fork
    ForkRevoked {
        fork_id: String,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkCreatedInfo {
    pub fork_id: String,
    pub worktree_path: String,
    pub fork_branch: String,
    pub tmux_session: String,
}
```

### 4.3 Fork Engine API

```rust
pub struct ForkEngine {
    /// Base directory for worktrees
    worktree_base: PathBuf,  // ~/.agent-hand/worktrees/
    /// Active forks registry
    forks: HashMap<String, GuestFork>,
}

impl ForkEngine {
    /// Create a fork from a host session
    /// 1. Resolve current HEAD of host's repo
    /// 2. Create git worktree at worktree_base/fork_id
    /// 3. Create new Instance and register in storage
    /// 4. Create ParentChild relationship
    /// 5. Start tmux session in worktree
    /// 6. Optionally launch AI tool
    pub async fn create_fork(
        &mut self,
        source: &Instance,
        guest_id: &str,
        tool: Option<Tool>,
        tmux: &TmuxManager,
        storage: &Arc<Mutex<Storage>>,
    ) -> Result<GuestFork>;

    /// Revoke and clean up a fork
    /// 1. Kill tmux session
    /// 2. Remove worktree (git worktree remove --force)
    /// 3. Delete orphan branch if not pushed
    /// 4. Remove Instance from storage
    /// 5. Remove relationship
    pub async fn revoke_fork(
        &mut self,
        fork_id: &str,
        tmux: &TmuxManager,
        storage: &Arc<Mutex<Storage>>,
    ) -> Result<()>;

    /// Check for expired forks and clean them up
    pub async fn cleanup_expired(
        &mut self,
        tmux: &TmuxManager,
        storage: &Arc<Mutex<Storage>>,
    ) -> Result<Vec<String>>; // returns cleaned fork IDs

    /// List all active forks for a given host session
    pub fn list_forks(&self, session_id: &str) -> Vec<&GuestFork>;

    /// Update last_activity timestamp (called when fork tmux session is active)
    pub fn touch(&mut self, fork_id: &str);
}
```

### 4.4 Git Operations

```rust
/// Git operations for fork worktree management
pub mod git_ops {
    /// Create a worktree for a guest fork
    /// Equivalent to:
    ///   cd <repo_path>
    ///   git worktree add <worktree_path> -b <branch_name> <commit>
    pub fn create_worktree(
        repo_path: &Path,
        worktree_path: &Path,
        branch_name: &str,
        commit: &str,
    ) -> Result<()>;

    /// Remove a worktree
    /// Equivalent to:
    ///   git worktree remove <worktree_path> --force
    pub fn remove_worktree(
        repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<()>;

    /// Get current HEAD commit of a repository
    pub fn resolve_head(repo_path: &Path) -> Result<String>;

    /// Get current branch name
    pub fn current_branch(repo_path: &Path) -> Result<String>;

    /// Delete a local branch (cleanup after worktree removal)
    /// Only deletes if branch was never pushed to a remote
    pub fn delete_orphan_branch(
        repo_path: &Path,
        branch_name: &str,
    ) -> Result<()>;

    /// List all worktrees for a repo
    pub fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeInfo>>;
}
```

### 4.5 Storage Schema Extension

The fork registry is stored alongside sessions in the profile storage:

```json
{
  "instances": [ /* ... existing ... */ ],
  "groups": { /* ... existing ... */ },
  "relationships": [ /* ... existing ... */ ],
  "guest_forks": [
    {
      "fork_id": "gf_abc123def4",
      "source_session_id": "a1b2c3d4e5f6",
      "source_commit": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
      "source_branch": "main",
      "guest_id": "viewer@email.com",
      "worktree_path": "/Users/host/.agent-hand/worktrees/gf_abc123def4",
      "fork_branch": "fork/viewer-email/2026-03-06-1",
      "tmux_session": "fork_gf_abc1",
      "instance_id": "x1y2z3w4a5b6",
      "tool": "Claude",
      "created_at": "2026-03-06T14:30:00Z",
      "last_activity": "2026-03-06T15:45:00Z",
      "expires_at": "2026-03-07T14:30:00Z",
      "status": "Active"
    }
  ],
  "fork_policy": "RequireApproval",
  "timestamp": "2026-03-06T15:45:00Z"
}
```

---

## 5. UI/UX Design

### 5.1 Fork Initiation (Viewer Side)

When in viewer mode, the status bar shows fork availability:

```
┌─────────────────────────────────────────────────────────────────────┐
│ ▶ LIVE  │  host: user@host  │  Ctrl+F: Fork  │  ?: Help          │
│─────────────────────────────────────────────────────────────────────│
│                                                                     │
│  [... terminal content from host session ...]                       │
│                                                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Pressing `Ctrl+F`** opens a fork dialog:

```
┌─────────────── Fork Session ──────────────┐
│                                            │
│  Source: main @ a1b2c3d (3 min ago)        │
│  Project: ~/projects/myapp                 │
│                                            │
│  AI Tool: [Claude ▾]                       │
│                                            │
│  A git worktree will be created at:        │
│  ~/.agent-hand/worktrees/gf_abc123/        │
│                                            │
│  Branch: fork/you@email/2026-03-06-1       │
│                                            │
│  ┌──────────┐  ┌──────────┐               │
│  │  Fork ⏎  │  │ Cancel   │               │
│  └──────────┘  └──────────┘               │
└────────────────────────────────────────────┘
```

### 5.2 Fork Approval (Host Side)

When policy is `RequireApproval`, the host sees a toast notification:

```
┌──────────────────────────────────────────────────────────┐
│  🍴 Fork request from viewer@email.com                   │
│  Session: myapp │ Branch: main @ a1b2c3d                 │
│  [A]pprove  [D]eny  [I]gnore                            │
└──────────────────────────────────────────────────────────┘
```

### 5.3 Fork Visibility (Host Session List)

Active forks appear as child sessions in the host's tree view:

```
  ▾ projects/myapp
    ● myapp (Running)                    ← host session
      🍴 fork/viewer@email (Active)      ← guest fork
      🍴 fork/another@user (Detached)    ← detached fork
    ○ myapp-tests (Idle)
```

The fork entry shows:
- `🍴` icon to distinguish from regular sub-sessions
- Guest identity
- Fork status (Active / Detached / Expired)
- Time since creation (on hover or detail view)

### 5.4 Fork Management (Host)

Selecting a fork and pressing `Enter` or `d` opens a management dialog:

```
┌──────────── Guest Fork: viewer@email ────────────┐
│                                                    │
│  Fork ID:    gf_abc123def4                         │
│  Guest:      viewer@email.com                      │
│  Source:     main @ a1b2c3d                         │
│  Branch:     fork/viewer-email/2026-03-06-1        │
│  Created:    2 hours ago                           │
│  Last Active: 15 min ago                           │
│  Expires:    in 22 hours                           │
│  Worktree:   ~/.agent-hand/worktrees/gf_abc123/    │
│                                                    │
│  [A]ttach (view fork terminal)                     │
│  [R]evoke (kill & cleanup)                         │
│  [E]xtend TTL (+24h)                               │
│  [C]ancel                                          │
└────────────────────────────────────────────────────┘
```

### 5.5 Keybindings

| Context | Key | Action |
|---------|-----|--------|
| Viewer mode | `Ctrl+F` | Open fork dialog |
| Session list (on fork) | `Enter` | Open fork management |
| Session list (on fork) | `d` | Quick revoke (with confirmation) |
| Fork management dialog | `a` | Attach to fork's tmux session |
| Fork management dialog | `r` | Revoke fork |
| Fork management dialog | `e` | Extend TTL by 24h |

### 5.6 Configuration (config.json)

```json
{
  "fork": {
    "policy": "require-approval",
    "default_ttl_hours": 24,
    "max_forks_per_session": 5,
    "max_total_forks": 20,
    "worktree_base": "~/.agent-hand/worktrees",
    "auto_launch_tool": true,
    "default_tool": "claude",
    "cleanup_on_start": true,
    "branch_prefix": "fork/"
  }
}
```

---

## 6. Implementation Strategy

### Phase 1: Core Fork Engine (Local Only)

**Goal**: Host can manually create forks from their own session list. No collaboration/viewer integration yet.

1. **Git worktree operations** (`src/session/git_ops.rs`)
   - `create_worktree()`, `remove_worktree()`, `resolve_head()`, `current_branch()`
   - Tests against a temporary git repo

2. **GuestFork data structure** (`src/session/guest_fork.rs`)
   - Struct definitions, serialization, status enum
   - Storage integration: extend `StorageData` with `guest_forks` field

3. **ForkEngine** (`src/session/fork_engine.rs`)
   - `create_fork()`: worktree creation → Instance creation → relationship → tmux session
   - `revoke_fork()`: tmux kill → worktree remove → storage cleanup
   - `cleanup_expired()`: scan for expired forks on app startup and periodically

4. **UI: Fork from session list**
   - Add "Fork" option to session context menu (when selecting own session)
   - Display fork children in tree view with `🍴` icon
   - Fork management dialog

**Deliverable**: A host can fork their own session into an isolated worktree, manage forks in the UI, and forks auto-expire.

### Phase 2: Viewer-Initiated Fork

**Goal**: A viewer watching via relay can fork into their own worktree on the host machine.

1. **Protocol extension** (`pro/src/collab/protocol.rs`)
   - Add `ForkRequest`, `ForkResponse`, `ForkCreated`, `ForkRevoked` to `ControlMessage`
   - Handle fork messages in `RelayClient`

2. **Viewer UI integration** (`src/ui/viewer.rs`)
   - `Ctrl+F` keybinding in viewer mode
   - Fork dialog with tool selection
   - Transition from viewer mode to fork session

3. **Host-side approval flow**
   - Toast notification for fork requests
   - Approval/deny keybindings
   - `ForkPolicy` configuration

4. **Post-fork viewer transition**
   - After fork is created, viewer can switch to fork's tmux session
   - Or continue watching host and access fork later

**Deliverable**: End-to-end viewer fork flow — viewer presses `Ctrl+F`, host approves, worktree is created, viewer can switch to their fork.

### Phase 3: Lifecycle & Polish

**Goal**: Production-ready fork management with proper lifecycle handling.

1. **Activity tracking**: Update `last_activity` by polling fork's tmux session status
2. **Expiry enforcement**: Background task checks for expired forks every 5 minutes
3. **Startup cleanup**: On app start, scan for orphaned worktrees and clean up
4. **Limits enforcement**: `max_forks_per_session`, `max_total_forks`
5. **Hook integration**: Emit hook events for fork creation, revocation, expiry
6. **Edge cases**: Handle fork of a fork, host session deletion while forks exist, worktree directory conflicts

---

## 7. Dependencies

### Depends On
- **SPEC-01 (Collaboration V2)**: Fork from viewer mode requires the viewer collaboration infrastructure
- **Existing session management**: `Instance`, `Relationship`, `Storage` from `src/session/`
- **Existing tmux management**: `TmuxManager`, `TmuxSession` from `src/tmux/`
- **Existing relay protocol**: `ControlMessage`, `RelayClient` from `pro/src/collab/`

### Enables
- **SPEC-03 (Presence & Cursor Tracking)**: Fork sessions can show where the guest is working relative to the host
- **SPEC-05 (ECS Runtime)**: Fork lifecycle events are natural ECS events (ForkCreated, ForkExpired, ForkRevoked)
- **SPEC-06 (Security Mechanism)**: Fork operations are classifiable by risk level (creating worktree = medium, revoking = low)
- **SPEC-07 (Memory & Relationships)**: Fork relationships carry rich context (source commit, purpose, outcome)

### External Dependencies
- `git` CLI (for worktree operations) — already implicitly required by the project
- No new crate dependencies anticipated; `std::process::Command` for git operations

---

## 8. Security Considerations

### 8.1 Filesystem Isolation

Git worktrees provide strong isolation:
- Each worktree has its own working directory with separate files
- Worktrees share the `.git` object store (read-only from worktree perspective)
- A guest cannot modify files in the host's working directory through git operations
- Worktrees are placed outside the project directory (`~/.agent-hand/worktrees/`) to prevent any path traversal concerns

### 8.2 Tmux Session Isolation

- Fork tmux sessions run under the same `agentdeck_rs` tmux server as host sessions
- The guest's AI agent runs with the host user's OS permissions (this is inherent to the tmux model)
- **Important**: Guest forks run with the host's user privileges. The fork mechanism is for trusted collaborators, not untrusted users.

### 8.3 Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Guest runs destructive commands | Fork is in isolated worktree; worst case is worktree corruption, not host corruption. Host can revoke at any time. |
| Worktree sprawl fills disk | `max_total_forks` limit + auto-expiry TTL + startup cleanup |
| Guest pushes to host's remote | Guest's fork branch has a distinct prefix (`fork/`). Push permissions are governed by git remote auth, not Agent Hand. |
| Fork of a fork creates deep nesting | Phase 3 addresses this — limit fork depth to 1 (no forking a fork) |
| Host deletes session with active forks | Cascade: revoke all forks before deleting host session |
| Orphaned worktrees after crash | Startup reconciliation: scan `worktree_base` for directories not in fork registry, clean up |

### 8.4 Trust Model

```
┌─────────────────────────────────────────────────────┐
│  Trust Boundary: Host's Machine                      │
│                                                       │
│  Host Session ──── full OS permissions                │
│       │                                               │
│       ├── Guest Fork A ── same OS user, isolated git  │
│       │                    worktree, separate branch   │
│       │                                               │
│       └── Guest Fork B ── same OS user, isolated git  │
│                            worktree, separate branch   │
│                                                       │
│  Key: Guest forks have filesystem isolation via git    │
│  worktrees but NOT OS-level privilege isolation.       │
│  This is a collaboration tool for trusted teams,      │
│  not a security sandbox.                              │
└─────────────────────────────────────────────────────┘
```

---

## 9. Open Questions

### Q1: Remote guest forks?
Currently, forks happen on the host's machine. Should we support a mode where the guest forks to their own machine? This would require the guest to have the repo cloned locally and would fundamentally change the architecture. **Recommendation**: Defer to a future spec. The local-fork model is simpler and more useful for the primary use case (pair programming on a shared machine or server).

### Q2: Fork branch naming collisions?
If the same guest forks the same session twice on the same day, the branch name `fork/guest@email/2026-03-06-1` could collide. **Proposed solution**: Append an incrementing counter: `-1`, `-2`, etc. Check for existing branches before creating.

### Q3: Worktree for non-git projects?
If the host's project is not a git repository, worktree isolation is not possible. **Proposed solution**: Return an error with a clear message. Fork requires a git repo. We could consider `cp -r` as a fallback in the future, but git worktree is the core mechanism.

### Q4: Guest identity verification?
The `guest_id` comes from the viewer's authentication during relay connection. For relay-based collaboration, this is the email/display name from `ViewerAuth`. For local forks (Phase 1), the host is forking for themselves, so identity is implicit. **Open**: Should we support any additional identity verification for fork requests?

### Q5: Fork notification to other viewers?
When a guest forks, should other viewers be notified? The `ForkCreated` broadcast message is defined in the protocol, but it could be noisy. **Recommendation**: Broadcast a brief notification; viewers can dismiss it. It adds to the "shared workspace" feeling.

### Q6: Shared clipboard / state between host and fork?
Should there be a mechanism for the host and a guest fork to share state beyond git (e.g., clipboard, environment variables, notes)? **Recommendation**: Defer. Git branches + push/pull are the natural collaboration mechanism. Additional state sharing adds complexity without clear benefit in v1.
