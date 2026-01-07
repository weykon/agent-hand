# 🦀 Agent Hand

> English: [README.md](README.md)

**多开 AI agent 窗口做 vibecoding 时很容易乱套？Agent Hand 帮你管理。**

一个基于 tmux 的快速终端会话管理器，用于 AI 编程代理（Claude / Copilot / OpenCode 等）。

> Agent Hand 是受原始 Go 开源项目
> [agent-deck](https://github.com/asheshgoplani/agent-deck) 启发的 Rust 重写版本。

![Preview](docs/preview.jpg)

## Why Agent Hand?

当你同时跑多个 AI agent（Claude、Copilot、OpenCode 等）做 vibecoding 时：
- 🤯 窗口太多，不知道哪个在等你确认、哪个跑完了
- 🔄 切来切去找不到刚才那个 session
- 😵 错过了 agent 的确认提示，白白等了半天

Agent Hand 解决这些问题：

| 状态图标 | 含义 | 你需要做什么 |
|---------|------|-------------|
| `!` 蓝色闪烁 | **需要确认** - agent 等你选 Yes/No | 赶紧去看！ |
| `●` 黄色动画 | **正在运行** - agent 在思考/执行 | 可以先做别的 |
| `✓` 青色 | **刚跑完** - 20分钟内完成的 | 去看看结果 |
| `○` 灰色 | **空闲** - 还没启动或已经看过了 | 随时可以继续 |

## Highlights

- **一目了然的状态列表**：所有 session 的状态实时显示
- **快速跳转**：`Ctrl+G` 弹出搜索框，秒切到任意 session
- **TUI dashboard**：运行 `agent-hand` 统一管理
- **分组管理**：按项目/用途组织你的 session
- **Session 标签**：自定义标题和颜色标签
- **tmux 加持**：`Ctrl+Q` 一键回 dashboard
- **自动升级**：`agent-hand upgrade`

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash
```

By default it installs to `/usr/local/bin` (if writable), otherwise `~/.local/bin`.

### Build from source

```bash
git clone https://github.com/weykon/agent-hand.git agent-hand
cd agent-hand
cargo build --release

# optional
cargo install --path .
```

## Quickstart

```bash
# open the TUI dashboard
agent-hand
```

From the dashboard:
- `n` create a session
- `Enter` attach
- in tmux: `Ctrl+Q` detach back to the dashboard
- in tmux: `Ctrl+G` popup → search + switch to another session

## Keybindings (TUI)

- Navigation: `↑/↓` or `j/k`, `Space` toggle expand/collapse group
- Session selected: `Enter` attach, `s` start, `x` stop, `r` edit (title/label), `R` restart, `m` move, `f` fork, `d` delete
- Group selected: `Enter` toggle, `g` create, `r` rename, `d` delete (empty = delete immediately; non-empty = confirm options)
- Global: `/` search, `p` capture preview snapshot, `?` help

## 自定义快捷键

启动时会读取 `~/.agent-hand/config.json`（也兼容旧目录 `~/.agent-deck-rs/config.json`）。

示例：

```json
{
  "keybindings": {
    "quit": ["q", "Ctrl+c"],
    "up": ["Up", "k"],
    "down": ["Down", "j"],

    "select": "Enter",
    "toggle_group": "Space",
    "expand": "Right",
    "collapse": "Left",

    "new_session": "n",
    "refresh": "Ctrl+r",
    "search": "/",
    "help": "?",

    "start": "s",
    "stop": "x",
    "rename": "r",
    "restart": "R",
    "delete": "d",
    "fork": "f",
    "create_group": "g",
    "move": "m",
    "preview_refresh": "p"
  }
}
```

支持的按键名：`Enter` / `Esc` / `Tab` / `Backspace` / `Space` / `Up` / `Down` / `Left` / `Right`，以及单个字符（如 `r`、`R`、`/`）。
修饰键：`Ctrl+` / `Alt+` / `Shift+`。

注意：目前仅影响主 dashboard（Normal 模式）；其它对话框仍使用固定按键。

### tmux 热键（Ctrl+G / Ctrl+Q）

这两个热键绑定在 agent-hand 的 **专用 tmux server**（`tmux -L agentdeck_rs`）上，不会影响你默认的 tmux server。

在 `~/.agent-hand/config.json` 增加：

```json
{
  "tmux": {
    "switcher": "Ctrl+g",
    "detach": "Ctrl+q"
  }
}
```

配置会在下次 attach 时生效（agent-hand 会在 attach 时重绑按键）。

如果你之前使用的是旧目录 `~/.agent-deck-rs/`，当 agent-hand 检测到新目录 `~/.agent-hand/` 里还没有任何 session 时，会在启动时自动把旧 profiles 迁移到新目录。

## CLI

```bash
# add a session (optional --cmd runs when starting the tmux session)
agent-hand add . -t "My Project" -g "work/demo" -c "claude"

# list sessions
agent-hand list

# status overview
agent-hand status -v

# start / attach
agent-hand session start <id>
agent-hand session attach <id>

# upgrade from GitHub Releases
agent-hand upgrade
```

## Notes

- Agent Hand uses a **dedicated tmux server** (`tmux -L agentdeck_rs`) so it won’t touch your default tmux.
- tmux preview capture is intentionally **cached by default**; press `p` to refresh the snapshot when needed.
- Global config lives under `~/.agent-hand/` (legacy `~/.agent-deck-rs/` is still accepted).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT
