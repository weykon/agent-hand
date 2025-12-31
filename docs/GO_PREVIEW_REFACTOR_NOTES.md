# Go 版 Preview/导航卡顿重构笔记（来自 Rust 端实践）

## 背景：为什么 preview 会“很卡”
在 macOS 上，`tmux capture-pane` 的成本往往远高于直觉：

- **每次调用都会 spawn 子进程**（`exec.Command("tmux", ...)`），进程创建+IPC 本身就有 10~50ms 级别开销。
- tmux 侧需要从 pane 的 scrollback/当前内容中截取并输出，若输出很活跃、包含大量 ANSI 控制序列、或 scrollback 很大，会进一步放大耗时。
- 如果把 `capture-pane` 放在 UI 的 Update/Render 高频路径里（例如每次上下键、每个 tick），用户就会感知明显卡顿（事件循环被阻塞）。

> 结论：preview 的“实时性”非常昂贵，但对实际使用价值往往不高（用户更关心“最后一眼”）。

---

## Rust 端采用的策略（建议 Go 端同步）

### 1) Preview 默认不实时：改为“快照缓存”
**默认情况下，切换选中项只显示缓存快照，不触发 tmux 调用。**

建议行为：
- 若选中 session 正在运行：
  - 若存在 `preview_cache[sessionID]`：直接显示（O(1) 字符串渲染）。
  - 若不存在缓存：显示提示，如 `Preview not cached. Press 'p' to capture snapshot.`
- 提供手动刷新键（例如 `p`）：
  - 执行一次 `capture-pane` 并写入 `preview_cache`。

这样“上上上”导航完全不会触发 tmux 子进程，UI 会非常稳。

### 2) 在用户“离开 tmux attach”时自动缓存一次（最后一眼）
用户真正关心的通常是 detach 回到 agent-deck 后看到的那份内容。

建议：
- 在 `attach-session` 返回（detach）后，**立刻抓一次 `capture-pane`** 写入 `preview_cache`。
- 这次抓取不需要很大：建议 80~200 行即可。

### 3) 状态更新别用全量 capture-pane：用 activity gating + 低频刷新
对于 status（WAITING/RUNNING）这种判断：
- 不要每 tick 对每个 session 都 `capture-pane`。
- 先用 `session_activity`/`display -p` 等轻量指标做“有变化才抓”的 gating。
- 并且在导航中（用户连续上下）暂停后台状态刷新（避免抢 CPU/IO）。

---

## 如果仍想做“实时 preview”，推荐可取消（Cancel）模式
当确实需要实时 preview 时（例如用户停住不动看内容）：

- 启动后台 goroutine 执行 `capture-pane`。
- 每次 selection 改变：
  - 取消上一轮 context（`context.WithCancel`），丢弃旧结果。
  - 只保留最后一次请求的结果落地到 `preview_cache`。

伪代码：

```go
var (
  previewCancel context.CancelFunc
  previewMu sync.Mutex
)

func requestPreview(sessionID string) {
  previewMu.Lock()
  if previewCancel != nil { previewCancel() }
  ctx, cancel := context.WithCancel(context.Background())
  previewCancel = cancel
  previewMu.Unlock()

  go func() {
    // 可选：debounce 150ms
    time.Sleep(150 * time.Millisecond)
    select { case <-ctx.Done(): return; default: }

    out, err := capturePane(ctx, sessionID) // exec.CommandContext
    if err != nil || ctx.Err() != nil { return }

    // 只写入最新请求的结果（可用版本号/原子指针保证）
    cache.Store(sessionID, out)
    sendMsg(previewUpdated{sessionID})
  }()
}
```

> 注意：`exec.CommandContext` 在 cancel 时会杀子进程，但 tmux server 端的负载仍可能存在；因此更推荐“默认快照 + 手动刷新”。

---

## 迁移清单（Go 端改动建议按顺序）
1. 抽象 `preview_cache map[sessionID]string`（先内存即可）。
2. preview 渲染只读 cache，不主动 `capture-pane`。
3. 加一个 `p` 键触发一次快照刷新。
4. attach 返回后自动快照一次。
5. 状态刷新加频率限制 + activity gating；导航中暂停后台更新。

---

## 取舍讨论点（你可以参与决策）
- **是否需要持久化 preview_cache 到 sessions.json？**
  - 优点：重启后仍有“最后一眼”。
  - 缺点：JSON 变大、写盘频繁；建议只在退出/定时（比如 10s+）落盘，且限制长度（最多 N 行/最多 M KB）。
- **快照长度**：建议 120 行起步，按体验调。
- **是否要做可取消实时 preview**：仅在确实需要“停住后自动更新”时再加。
