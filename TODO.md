# TODO: Session 创建流程改造

目标：简化创建 session 的交互（默认 custom），移除 Claude/其他 CLI 选择，把体验集中在「路径选择/匹配」与「可选分组(group)列表选择」。

---

## 设计目标（用户体验）

- [ ] 创建 session 时默认使用 `custom`（不再出现 claude / 其他 provider 的选择步骤）
- [ ] 路径输入支持：
  - [ ] 从历史/候选列表选择（↑↓/jk + Enter）
  - [ ] 也允许直接手输路径（Enter 确认）
  - [ ] 路径候选的匹配/过滤做得更好（稳定、可预期、不会乱跳）
- [ ] 进入终端后默认 `cd` 到用户选择/输入的路径
- [ ] 可选设置 group：
  - [ ] group 用列表展示（支持过滤 + 选择）
  - [ ] 支持选择 `(none)` 表示不分组

---

## CLI 行为调整

- [ ] `agent-hand add`：默认走 custom（不要求/不提示选择 claude 等）
- [ ] 清理/隐藏与 provider 选择相关的 flags、提示文案与 help（如果仍需兼容，先做 deprecated 处理）
- [ ] 确认 session 启动时的工作目录：
  - [ ] tmux pane / command 入口确保执行 `cd <path>`（而不是仅在 UI 上记录路径）

---

## TUI：New Session Dialog 改造

- [ ] 移除/隐藏 “选择 claude / 其他 CLI” 相关 UI
- [ ] 路径选择体验：
  - [ ] 输入框 + 候选列表（建议保留现有 auto-suggest，但专注于路径匹配质量）
  - [ ] Enter：应用候选 / 进入下一步 / 提交（逻辑清晰，不要让用户猜）
- [ ] 新增 group 选择步骤：
  - [ ] 用和 MoveGroup 类似的「过滤 + 列表选择」组件
  - [ ] 支持 `Esc/Ctrl+C` 取消
  - [ ] footer 文案保持一致

---

## 数据与持久化

- [ ] Session 数据中明确区分：
  - [ ] `workdir` / `path`（用于 `cd`）
  - [ ] `group_path`（可空）
  - [ ] `command`/`runner` 固定为 custom（或可推导）
- [ ] 存量 sessions 的兼容迁移：
  - [ ] 旧字段仍可读取
  - [ ] 新建时不再写入 provider 相关字段（或写入默认值）

---

## 测试与验收

- [ ] 单测：路径匹配/过滤逻辑（边界：空输入、相对路径、大小写、~ 展开策略如有）
- [ ] 单测：创建 session 后启动命令包含 `cd <path>`
- [ ] 手测：
  - [ ] 新建 session：选路径 → (可选)选 group → 创建 → attach 后 cwd 正确
  - [ ] group 选择：过滤、上下选择、(none)

---

## 交付拆分建议（commit 粒度）

- [ ] docs: 说明创建流程变更（去 provider 选择，强调路径/分组）
- [ ] cli: default custom + 去除 provider 选择
- [ ] tui: new session path-focused
- [ ] tui: group selectable list in new session
- [ ] runtime: ensure tmux command starts with `cd <path>`
