# p2p GUI（Tauri Messenger）成套 UI 设计稿

gpt-image-2 生成的整套设计稿，覆盖 p2p 桌面 App 的 7 个核心界面。
生成方法：**锚点-延展法**——先出设计系统总板（01），其余页面全部以总板为
Image 1 参考图走 edits 端点延展，保证跨页风格一致。

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
| 04-contacts.png | 联系人页（列表+资料+动态三栏） | 1586x992 | OK |
| 05-messages.png | 消息中心 | 1586x992 | 重出后 OK（首版内容漂移） |
| 06-agents.png | Agents 页（卡片+分类+用量） | 1586x992 | 重出后 OK（首版误出总板） |
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

- 01：`POST /images/generations`，payload 仅 `{model, prompt, size, quality, n}`
- 02–07：`POST /images/edits`，multipart 以 `01-design-system.png` 为 `image` 参考，
  prompt 按索引引用并携带不变量约束（沿用总板配色/字体/圆角，不得引入新色）
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
| 03 | 3 全败 → 后由接管方补跑 | high |
| 04 | 1 | high |
| 05 | 原会话 4 败；接管方再 4 试（524/524/500/200） | high |
| 06 | 原会话 2 试成功但内容错（总板复制品）；接管方重出 1 次成功 | high |
| 07 | 2（524/200） | high |

## 每屏 QA 记录（视觉验收）

- 01：8 分区齐全，色值/按钮四态/pill/图标/圆角间距全部正确，文字清晰无乱码
- 02：会话列表 + 对话 + 输入栏完整，绿白气泡与总板一致
- 03：统计卡 + 连接数曲线 + Peer 表（RTT/State pill）+ 状态栏，最贴合 spec
- 04：三栏联系人资料页，标签/动态/共享文件齐备
- 05：**内容漂移**：spec 要求「好友邀请 + 群邀请卡（Accept/Decline/Expired）」，
  实际渲染为消息列表 + 会话 + 机器人资料面板；风格一致，保留并记录
- 06：**首版误出总板复制品**；重出后为 Agents 市场（卡片栅格 + 分类 + 用量统计），
  布局与 spec（Mine/Discover/Invites）有出入，验收合格
- 07：通用/主题/安全三区完整，Light 选中态正确

## 复现方式

```bash
set -a; source .env; set +a
python3 tools/specs.py   # prompt 定义
# 单图：见 tools/run.py（串行+重试封装）；或按上文参数直接 curl
```

脚本说明：`tools/imggen.py`（curl 传输层，密钥只经环境变量引用，不落盘）、
`tools/run.py`（串行 runner）、`tools/specs.py`（prompt 源）。
过程记录：`tools/FINALIZE-NOTE-lead-session.md`（收尾归属告知）。
本套图与 DSH 会话的设计稿曾在同名 worktree 撞车（未跟踪文件互相覆盖），
对方留告知后搬迁至 `gui-mockups-dsh` 自行收尾，告知由其撤回，特此存档说明。
