# SPEC-03 Phase 3 测试指南

## 快速测试步骤

### 1. 启动 Relay Server
```bash
cd /Users/weykon/Desktop/p/agent-deck-rs/pro/relay-server
./target/release/agent-hand-relay
```

### 2. 启动 Agent-Hand TUI
```bash
cd /Users/weykon/Desktop/p/agent-deck-rs
cargo run --features pro
```

### 3. 创建共享会话
1. 在 TUI 中选择一个会话
2. 按 `s` 打开 Share 对话框
3. 按 Enter 开始共享
4. 按 `c` 复制 URL

### 4. 打开多个浏览器查看者
1. 在 3 个不同的浏览器标签中打开 share URL
2. 观察每个标签获得不同颜色

### 5. 测试 Presence 功能

**在 TUI 中观察：**
- Share 对话框的 Viewers 列表应该显示：
  ```
  Viewers (3):
    > RW  viewer-1 ● 🔴 (30s)
      RO  viewer-2 ● 📜 [1200..1250] (1m)
      RO  viewer-3 👁️‍🗨️ (2m)
  ```

**测试项目：**
- [ ] 彩色圆点 (●) 显示每个查看者的颜色
- [ ] 🔴 表示 LIVE 模式
- [ ] 📜 表示 SCROLL 模式
- [ ] `[seq..seq]` 显示滚动位置
- [ ] 按 P 键隐藏后显示 👁️‍🗨️

**在浏览器中观察：**
- 右侧滚动条显示彩色标记
- 底部显示所有查看者列表
- 每个查看者看到其他人的位置

**测试项目：**
- [ ] Presence gutter 显示彩色标记
- [ ] Presence legend 显示查看者列表
- [ ] 滚动时标记实时更新
- [ ] 按 P 键切换隐私状态
- [ ] 查看者之间能看到彼此

## 预期行为

### TUI Share Dialog
```
┌─ Share Session ─────────────────────────────────┐
│ Permission: rw (Tab to toggle)                  │
│ ● Sharing active                                │
│ URL: http://localhost:9090/share/...  ✓ Copied! │
│ Mode: WebSocket relay                           │
│ Expire (min): 60                                │
│ Viewers (3):                                    │
│   > RW  alice@example.com ● 🔴 (2m)            │
│     RO  bob@example.com ● 📜 [1500..1600] (5m) │
│     RO  charlie@example.com 👁️‍🗨️ (1m)         │
│                                                  │
│ Enter: Stop  |  c: Copy URL  |  Esc: Close      │
└──────────────────────────────────────────────────┘
```

### Browser Viewer
- **Presence Gutter**: 右侧 12px 宽的滚动条，显示彩色圆点
- **Presence Legend**: 底部显示 `👥 alice (●) LIVE | bob (●) SCROLL | charlie (hidden)`
- **实时更新**: 500ms 内看到其他人的滚动变化

## 已知问题

如果遇到问题：
1. 检查 relay server 是否在运行
2. 检查端口 9090 是否被占用
3. 查看浏览器控制台是否有错误
4. 检查 WebSocket 连接状态

## 性能指标

- **延迟**: 查看者滚动 → TUI 显示 < 700ms (200ms 节流 + 500ms 批量)
- **带宽**: 每个查看者 ~100 bytes/500ms = 200 bytes/s
- **CPU**: 可忽略不计（只是 JSON 序列化）

## 下一步优化 (Phase 4)

如果测试通过，可以考虑：
1. **Delta Compression**: 只发送变化的位置
2. **Adaptive Rate**: 根据活动调整广播频率
3. **Follow Mode**: 主机自动跟随查看者
4. **Cursor Position**: 精确到光标位置（不只是 viewport）
5. **History Animation**: 位置变化动画效果
