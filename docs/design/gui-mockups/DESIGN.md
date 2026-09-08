# p2p GUI（Tauri Messenger）成套 UI 设计稿

gpt-image-2 生成的整套设计稿，覆盖 p2p 桌面 App 的 7 个核心界面。
生成方法：**锚点-延展法**——先出设计系统总板（01），其余页面两条通道延展：
edits 端点以总板为 Image 1 参考图；或 generations 端点在 prompt 内联总板
token 锚定（网关抖动期与 edits 内容塌缩时的更稳替代，见下文）。

- 生成日期：2026-09-08 深夜 ~ 2026-09-09 凌晨
- 模型：`gpt-image-2`（中转端点，密钥走 `.env` 的 `OPENAI_API_KEY` / `OPENAI_BASE_URL`）
- 布局依据：`apps/gui/src/config/menu.def.ts`、`docs/design/app-shell-redesign.md`、
  `apps/gui` 各 view（chat/contacts/network/messages/agents/settings）
- 品牌基调：浅色主题 + 微信绿 `#07C160`，Tauri 桌面窗口范式

## 文件清单

| 文件 | 内容 | 尺寸 | 首版结果 |
|---|---|---|---|
| 01-design-system.png | 设计系统总板（8 分区） | 1536x960 | OK |
| 02-chat.png | 聊天页（会话列表+对话） | 1536x960 | OK |
| 03-network.png | 网络页（Peers/Relay/图表/状态栏） | 1536x960 | OK |
| 04-contacts.png | 联系人页（三区树+详情：身份 chip/Capabilities/Danger Zone） | 1586x992 | 重出后 OK（首版漂移为通用资料页） |
| 05-messages.png | 消息中心（好友/群邀请，Accept/Decline/Expired） | 1586x992 | 补跑后 OK（批次 4 败于网关） |
| 06-agents.png | Agents 页（Mine/Discover/Invites 三节） | 1586x992 | 重出后 OK（首版塌缩为总板复制品） |
| 07-settings.png | 设置页（通用/主题/安全） | 1586x992 | OK |

## 设计 Token（总板定稿）

| Token | 值 | 用途 |
|---|---|---|
| BG-0 | `#FFFFFF` | 页面底色 |
| BG-1 | `#F7F8FA` | 卡片/分区底 |
| BG-2 | `#EFF1F3` | 输入框/嵌底 |
| PRIMARY | `#07C160` | 主操作/在线/选中（微信绿） |
| INFO | `#3B82F6` | 信息蓝 |
| WARNING | `#F59E0B` | 连接中/警示 |
| ERROR | `#EF4444` | 错误/离线警示 |
| DARK BG | `#252525` | 暗色模式底（后续扩展） |
| 圆角 | 6 / 10 / 16 | 控件 / 卡片 / 大容器 |
| 间距 | 4 / 8 / 12 / 16 / 24 | 间距刻度 |
| 字号 | 28 / 20 / 14 / 11 | Display / Title / Body / Caption |
| 图标 | 1.5px 线性 | Chat/Users/Network/Bell/Book/Share/Sparkles/Gear |

组件语言：左侧 56px 图标 Rail（Settings 沉底）、顶栏窗口题 `p2p` +
`Node Online` 绿 pill + Start/Stop 开关、底栏状态条
（Running / Port 47101 / 12 connections / v0.1.3）、状态 pill
（Online 绿 / Connecting 琥珀 / Offline 灰 / Error 红）。

## 生成方式与参数

- 01 总板：`POST /images/generations`，payload 仅 `{model, prompt, size, quality, n}`
- 02/07：`POST /images/edits`，multipart 以 `01-design-system.png` 为 `image` 参考，
  prompt 按索引引用并携带不变量约束（沿用总板配色/字体/圆角，不得引入新色）
- 03/05 补跑与 04/06 重出：`POST /images/generations` + prompt 内联 token 锚定，
  并加反设计板负约束（"this is a real application screen, NOT a design-system
  board"）。04/06 经 edits 首版分别漂移为通用资料页/塌缩为总板复制品，改道后
  一次命中；03 在 edits 下三连 524，改道后一次成功——**页面类优先 generations+内联锚定**
- 公共参数：`quality=high`、`n=1`；尺寸 `1536x960` 为主（中转对部分请求返回 1586x992）
- prompt 全文见 `tools/specs.py`（RAIL/TOPBAR/STATUSBAR/COMMON 组合式拼装）

## 网关实况与重试策略（重要经验）

中转为 Cloudflare 前置，源站慢且抖动：边缘约 127s 掐断（HTTP 524），
偶发 `500 upstream_error`。实测规律：

- 失败与参数无关，纯网关过载；**同参数重试即可成功**（本套图成功率 100%，重试 1~4 次）
- 重试节奏：失败后 sleep 15s；单张上限 3~5 次；high 连续失败可降 `quality=low`
  出草稿（05 曾用，后已用 high 重出替换）
- 严格串行，一张成功再发下一张（并发互相挤挂）
- 各张实际尝试次数见 `tools/takeover_log.json` 与 DESIGN.md 本表：

| 图 | 尝试次数 | 结果 |
|---|---|---|
| 01 | 3（500/超时/200） | high |
| 02 | 3（524x2/200） | high |
| 03 | 批次 edits 3 全败；补跑 generations 1 试成功 | high |
| 04 | 批次 edits 成功但内容漂移；重出 generations 2 试（524/200） | high |
| 05 | 批次 edits 3 败+low 1 败；补跑 4 败；末轮 2 试（500/200） | high |
| 06 | 批次 edits 成功但内容塌缩；重出 generations 1 试成功 | high |
| 07 | 2（524/200） | high |

## 每屏 QA 记录（视觉验收）

- 01：8 分区齐全，色值/按钮四态/pill/图标/圆角间距全部正确，文字清晰无乱码
- 02：会话列表 + 对话 + 输入栏完整，绿白气泡与总板一致
- 03：统计卡 + 连接数曲线 + Peer 表（RTT/State pill）+ 状态栏，最贴合 spec
- 04：三区树（FRIENDS(12)/GROUPS(4)/AGENTS(3)）+ 详情（`p2p://alice@key8f2k`
  身份 chip、Capabilities 绿 chip、Danger Zone 红 Remove、Send Message），命中 spec
- 05：Messages badge 3；Friend Invites（Dave/Erin）与 Group Invites
  （p2p-dev by Alice、group 'lab' Expired pill）+ Accept/Decline 按钮，命中 spec
- 06：Mine(2)/Discover/Invites 三节 + skills chips（deploy/monitor、i18n/review、
  pytest）+ Create Agent 按钮，rail Agents 高亮，命中 spec
- 07：通用/主题/安全三区完整，Light 选中态正确

## 复现方式

```bash
set -a; source .env; set +a
python3 tools/generate.py                    # 生成全部缺失页（串行+重试）
python3 tools/generate.py --only 03-network  # 单页重出
python3 tools/generate.py --mode edits       # 改用总板参考图 edits 通道
python3 tools/generate.py --fallback-low     # 失败时允许 low 草稿降级
```

脚本说明：`tools/generate.py`（最终版串行 driver：01 走 generations 总板，
页面默认 generations+内联锚定，`--mode edits` 可切参考图通道；密钥经临时
curlrc 传入，不进进程参数、不落日志）、`tools/specs.py`（prompt 源）、
`tools/takeover_log.json`（首轮接管生成的逐次尝试记录）。
历史：`tools/FINALIZE-NOTE-lead-session.md`（收尾归属告知）；DSH 会话的
imggen.py/run.py 赛马驱动与本次套图不配套，已移除（形状见 generate.py）。
本套图与 DSH 会话的设计稿曾在同名 worktree 撞车（未跟踪文件互相覆盖），
对方留告知后搬迁至 `gui-mockups-dsh` 自行收尾，告知由其撤回，特此存档说明。
