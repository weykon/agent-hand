use std::path::Path;

use crate::Result;

/// Name of the manager meta-skill directory.
const MANAGER_DIR: &str = "_manager";

/// Generate the `_manager/SKILL.md` meta-skill inside the skills repo.
///
/// This skill teaches AI agents how to use `agent-hand skills` CLI commands
/// to manage the skills library.
pub fn generate_manager_skill(repo_path: &Path) -> Result<()> {
    let dir = repo_path.join(MANAGER_DIR);
    std::fs::create_dir_all(&dir)?;

    let content = manager_skill_content();
    std::fs::write(dir.join("SKILL.md"), content)?;

    Ok(())
}

fn manager_skill_content() -> String {
    r#"---
name: _manager
description: Meta-skill for managing the agent-hand skills library
---

# Skills Manager

You can manage skills using the `agent-hand skills` CLI. This meta-skill
teaches you the available commands.

## Commands

### Initialize skills repository
```bash
agent-hand skills init --repo <repo-name>
```
Creates a private GitHub repository for storing skills and clones it locally.
Default repo name: `agent-skills`.

### Sync skills from GitHub
```bash
agent-hand skills sync
```
Pulls the latest changes from the remote skills repository.

### List all skills
```bash
agent-hand skills list
agent-hand skills list --json
```
Shows all discovered skills. Use `--json` for machine-readable output.

### Link a skill to the current project
```bash
agent-hand skills link <skill-name>
agent-hand skills link <skill-name> --group <group-name>
```
Creates a symlink from the skill in the repo to the current project's
CLI-specific commands directory (e.g., `.claude/commands/`).

The CLI type is auto-detected based on which config directories exist
in the project (`.claude/`, `.agents/`, `.cursor/`).

### Unlink a skill
```bash
agent-hand skills unlink <skill-name>
```
Removes the skill symlink from the current project.

### Add a community skill
```bash
agent-hand skills add <github-url>
```
Clones a community skill from a GitHub URL into the local skills repo.

### Push changes
```bash
agent-hand skills push
```
Commits and pushes all local skill changes to GitHub.

## Creating a New Skill

1. Create a directory in your skills repo (e.g., `my-skill/`)
2. Add a `SKILL.md` file with YAML frontmatter:

```markdown
---
name: my-skill
description: What this skill does
---

# My Skill

Instructions for the AI agent...
```

3. Run `agent-hand skills sync` to discover the new skill
4. Run `agent-hand skills link my-skill` to use it in a project

## Directory Structure

Skills are stored in `~/.agent-hand/skills/repo/` and organized freely:

```
repo/
  global/
    code-review/SKILL.md
    testing/SKILL.md
  python/
    django/SKILL.md
  _manager/SKILL.md          (this file)
```
"#
    .to_string()
}
