# make-sound-pack — 为 agent-hand 制作音效包

你是在帮用户完成一个完整的音效包制作流程：**找资源 → 下载 → 整理成 CESP 格式 → 上传 GitHub → 在 app 内安装**。

这是一个**动手 skill**，不只是说明文档——你要实际运行命令、下载文件、生成 manifest、推送到 GitHub。

---

## 第一步：了解用户想要什么

读取当前对话上下文。用户可能已经提到了主题或来源。尽量从上下文推断，不要重复问已知的信息。

需要确认以下几点（自然对话，不是问卷）：

1. **主题是什么？** 哪个游戏/角色/风格？（比如：英雄联盟-锐雯、星际争霸战斗音效、CS1.6等）
2. **语言版本？** 中文/英文/日文（针对有多语言的来源）
3. **包名是什么？** 给 GitHub 目录用的英文小写+下划线名称（比如 `riven_zh`）
4. **发布到哪里？** 默认发布到 `weykon/agent-hand-packs`。也可以推到 `PeonPing/og-packs`（需要 PR）。

---

## 第二步：找资源

根据主题给出最合适的资源来源：

### 英雄联盟 (League of Legends) — Community Dragon

Community Dragon 是社区维护的 LoL 资源镜像，**完全公开，无需登录**。

**URL 模式：**
```
# 选英雄语音 (champion select)
https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/{locale}/v1/champion-choose-vo/{champion_id}.ogg

# 禁英雄语音 (ban)
https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/{locale}/v1/champion-ban-vo/{champion_id}.ogg
```

**locale 参数：**
| 语言 | locale |
|------|--------|
| 中文 | `zh_cn` |
| 英文 | `default` |
| 日文 | `ja_jp` |
| 韩文 | `ko_kr` |

**常用英雄 ID：**
| 英雄 | ID |
|------|----|
| 锐雯 Riven | 92 |
| 流浪法师 Ryze | 13 |
| 盖伦 Garen | 86 |
| 伊泽瑞尔 Ezreal | 81 |
| 卡莎 Kai'Sa | 145 |
| 亚索 Yasuo | 157 |
| 薇恩 Vayne | 67 |

**查找任意英雄 ID：**
浏览 `https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/default/v1/champion-choose-vo/` 列出所有可用文件。

**验证链接是否有效（先测试）：**
```bash
curl -I "https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/zh_cn/v1/champion-choose-vo/92.ogg" | grep -i "content-type\|HTTP"
```

---

### 其他音效来源

**Freesound.org** — 开源音效库，Creative Commons 授权：
```bash
# 搜索（需要 API key，或直接在网页找到直链后用 curl 下载）
# 网址：https://freesound.org
# 下载后注意查看许可证：CC0（无限制）> CC BY（需署名）> CC BY-NC（不可商用）
```

**星际争霸 SC2 — CASC 提取器（需要本地游戏安装）：**
```bash
brew install casc-explorer  # macOS
# 在游戏目录用工具提取 .ogg/.wav 文件
```

**本地 WAV/OGG 文件（已有的文件直接用）：**
```bash
ls ~/.openpeon/packs/  # 查看已安装的包，可以复用里面的文件
```

---

## 第三步：下载声音文件

创建工作目录并下载：

```bash
# 替换 PACK_NAME 为你的包名
PACK_NAME="riven_zh"
WORK_DIR="/tmp/agent-hand-packs/$PACK_NAME/sounds"
mkdir -p "$WORK_DIR"

# 下载（示例：锐雯中文语音，champion_id=92）
curl -L "https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/zh_cn/v1/champion-choose-vo/92.ogg" \
     -o "$WORK_DIR/choose.ogg"

curl -L "https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/zh_cn/v1/champion-ban-vo/92.ogg" \
     -o "$WORK_DIR/ban.ogg"

# 验证下载
ls -lh "$WORK_DIR/"
```

**格式支持：** agent-hand 使用 `rodio` 播放，支持 `.ogg`、`.wav`、`.mp3`、`.flac`。OGG 体积最小，推荐优先用 OGG。

---

## 第四步：创建 CESP Manifest (openpeon.json)

在 `/tmp/agent-hand-packs/$PACK_NAME/` 创建 `openpeon.json`：

```json
{
  "name": "锐雯 (Riven) — 中文语音",
  "description": "英雄联盟锐雯中文选英雄台词作为编码事件音效。",
  "version": "1.0.0",
  "author": "你的名字",
  "source": "Community Dragon (社区维护的公开游戏数据)",
  "categories": {
    "session.start":    { "sounds": [{ "file": "choose.ogg" }] },
    "task.complete":    { "sounds": [{ "file": "choose.ogg" }] },
    "input.required":   { "sounds": [{ "file": "ban.ogg" }] },
    "task.error":       { "sounds": [{ "file": "ban.ogg" }] },
    "task.acknowledge": { "sounds": [{ "file": "choose.ogg" }] },
    "resource.limit":   { "sounds": [{ "file": "ban.ogg" }] },
    "user.spam":        { "sounds": [{ "file": "ban.ogg" }] }
  }
}
```

### 全部 CESP 事件类型

| 事件 | 触发时机 |
|------|---------|
| `session.start` | Agent 开始工作（从空闲变为运行中） |
| `task.complete` | 任务完成（从运行中变为空闲） |
| `input.required` | Agent 等待用户输入（变为 Waiting 状态） |
| `task.error` | 工具调用失败 |
| `task.acknowledge` | 已运行的 session 收到新 prompt |
| `resource.limit` | Context window 即将压缩 |
| `user.spam` | 5秒内连发 3+ 条消息检测到刷屏 |

**Tips：**
- 不需要全部 7 个事件，不填的类别会静音
- 一个类别可以放多个声音文件，运行时会随机选一个
- `file` 路径相对于包根目录，`sounds/` 前缀可选（有 `/` 则按原路径，否则从 `sounds/` 找）

---

## 第五步：验证本地效果

在发布之前，先在本地测试：

```bash
# 复制到 ~/.openpeon/packs/ 测试
cp -r /tmp/agent-hand-packs/$PACK_NAME ~/.openpeon/packs/

# 用 afplay/aplay 直接测试声音文件
afplay ~/.openpeon/packs/$PACK_NAME/sounds/choose.ogg   # macOS
aplay  ~/.openpeon/packs/$PACK_NAME/sounds/choose.ogg   # Linux
```

在 agent-hand 里：进入 `Settings > Sound`，将 `Sound Pack` 改为你的包名，然后触发一个 agent 任务来测试。

---

## 第六步：上传到 GitHub

```bash
# 克隆仓库
cd /tmp
gh repo clone weykon/agent-hand-packs agent-hand-packs-publish
cd agent-hand-packs-publish

# 复制包
cp -r /tmp/agent-hand-packs/$PACK_NAME .

# 提交并推送
git add $PACK_NAME/
git commit -m "feat: add $PACK_NAME sound pack"
git push origin main
```

推送成功后，包会出现在 agent-hand 的 **Settings > Sound > Install Packs** 浏览器里。

### 如果想贡献到社区（PeonPing/og-packs）

```bash
# Fork + PR 流程
gh repo fork PeonPing/og-packs --clone
cd og-packs
cp -r /tmp/agent-hand-packs/$PACK_NAME .
git add $PACK_NAME/
git commit -m "feat: add $PACK_NAME pack"
git push origin main
gh pr create --title "Add $PACK_NAME sound pack" \
             --body "来自 Community Dragon 的公开音效，符合 CESP 格式。"
```

---

## 第七步：在 app 内安装

1. 打开 agent-hand
2. 按 `s` 进入 Settings
3. 找到 **Sound > Install Packs**
4. 在列表里找到你的包，按 `i` 安装
5. 安装后在 **Sound Pack** 字段填入包名，保存

或者手动：已经 `cp -r` 到 `~/.openpeon/packs/` 就直接可用，无需安装步骤。

---

## 快速模板

如果用户只是想快速开始，给出这段可以直接运行的脚本，让他们只改 3 个变量：

```bash
#!/bin/bash
# === 修改这 3 个变量 ===
PACK_NAME="my_pack"          # 包目录名（小写+下划线）
CHAMP_ID="92"                # 英雄 ID
LOCALE="zh_cn"               # 语言 (zh_cn / default / ja_jp / ko_kr)
# =======================

BASE="https://raw.communitydragon.org/pbe/plugins/rcp-be-lol-game-data/global/$LOCALE/v1"
DIR="/tmp/agent-hand-packs/$PACK_NAME/sounds"
mkdir -p "$DIR"

curl -L "$BASE/champion-choose-vo/$CHAMP_ID.ogg" -o "$DIR/choose.ogg"
curl -L "$BASE/champion-ban-vo/$CHAMP_ID.ogg"    -o "$DIR/ban.ogg"

cat > "/tmp/agent-hand-packs/$PACK_NAME/openpeon.json" << EOF
{
  "name": "$PACK_NAME",
  "source": "Community Dragon",
  "categories": {
    "session.start":  { "sounds": [{ "file": "choose.ogg" }] },
    "task.complete":  { "sounds": [{ "file": "choose.ogg" }] },
    "input.required": { "sounds": [{ "file": "ban.ogg" }] },
    "task.error":     { "sounds": [{ "file": "ban.ogg" }] }
  }
}
EOF

echo "✓ Pack ready at /tmp/agent-hand-packs/$PACK_NAME"
ls -lh "$DIR/"
```

---

## 交互风格

- **直接做事**：不只是给说明，要运行命令、下载文件
- **确认关键步骤**：下载前告诉用户 URL，推送前让用户确认
- **遇到 404**：说明该英雄/语言可能没有对应语音，推荐替代（比如换 `default` locale）
- **遇到权限问题**：检查是否已 `gh auth login`
- **遇到格式不支持**：用 `ffmpeg` 转换 (`ffmpeg -i input.mp3 output.ogg`)
