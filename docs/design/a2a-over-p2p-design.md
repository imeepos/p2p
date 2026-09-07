# A2A over P2P：Agent 智能体发布 / 发现 / 直连聊天 —— 设计方案 v0

> 状态: 草案（待审查定稿） | 日期: 2026-09-08
> 依赖: [p2p-base-design.md](p2p-base-design.md)（底座分层）、[wire-protocol.md](wire-protocol.md)（协议 ID 与帧）、
> [acp-over-p2p-design.md](acp-over-p2p-design.md)（agent 托管/桥约定）、[idle-token-sharing-plan.md](idle-token-sharing-plan.md)（签名声明模式）
> 定位: 底座之上的应用层方案。不改通信内核，只新增应用协议与 apps/ + crates/ 组件。
> 参照实现: Google A2A 协议（a2a-protocol.org，Agent Card / Task / Message-Part，JSON-RPC 2.0 语义）

把 Google A2A（Agent2Agent）协议的能力声明与任务语义嫁接到 p2p-base 底座之上：
节点创建**公开/私有** agent 智能体，经 P2P 通知其他节点，其他节点**发现 agent 能力**后
**直接与之聊天**。聊天走 A2A 任务语义（task + message/part），宿主复用 acp-agent 的
dsh 子进程托管资产（监狱/mcpServers 改写/权限瀑布）。

## 1. 背景与目标

### 1.1 A2A 协议要点（本方案采纳的子集）

| A2A 概念 | 语义 | 本方案落点 |
|---|---|---|
| Agent Card | agent 能力自述：name/description/url/capabilities/skills/securitySchemes | 卡片模型 + 签名信封（§4） |
| Task | 工作单元，生命周期 submitted→working→completed/failed/cancelled/input_required/auth_required/rejected | /a2a/1 task 相（§5.2） |
| Message/Part | role(user/agent) + parts：TextPart/FilePart/DataPart | task 内消息体（§5.3） |
| 发现 | 拉取 Agent Card（标准是 HTTP well-known/registry） | rendezvous 命名空间 + mDNS + 已连 peer 推卡（§7） |
| 传输 | JSON-RPC 2.0 over HTTP | p2p 协议流（QUIC/TCP，varint 帧）替代 HTTP 传输 |

A2A 标准的 url 字段在 p2p 世界落成 **a2a://<hostPeerId>/<agentId>** 寻址；
securitySchemes 由底座传输层互认身份替代（不另发明 OAuth）。

### 1.2 目标

1. 节点可创建 agent 智能体，声明**公开或私有**可见性。
2. 公开 agent 经 P2P 通知全网可发现；私有 agent 经邀请帧点对点通知指定节点。
3. 其他节点发现 agent 的**能力**（skills/capabilities）后可一键**直连聊天**。
4. 聊天走 A2A 任务语义，宿主复用 acp-agent 的 dsh 子进程托管（零重复造轮子）。
5. p2p-base 底座一层不侵：A2A 是底座的应用，不是底座的扩展（协议 ID 常量登记除外）。

### 1.3 非目标

- 不做 agent 间自动协商/编排（那是 E10 llm-share 与后续 agent 编排的课题）；本方案只做
  「人（GUI）↔ 远程 agent」的 A2A 聊天 + 能力发现。
- 不做 A2A 全量规范实现（streaming 之外的 pushNotifications/artifact 等按需裁剪）。
- 不做 agent 市场/评分/信誉体系。
- 不承诺发现「全部」agent；发现目标收敛为「找到并聊上可用的 agent」（对齐 social-discovery-plan 语义）。

## 2. 立场与总架构

**一句话架构**：Rust 纯库 `crates/a2a` 定义 AgentCard/签名信封/AgentBook/task 帧（零网络零进程）；
宿主扩展 `apps/acp-agent` 挂 /a2a/1 handler（card 相 + task 相），task 桥接到既有 dsh
子进程的 ACP session；GUI 新增 `/agents` 页（消息页面模式）做发布/发现/管理，聊天页新增
A2A 会话 kind（复用现有气泡渲染，A2A parts 适配到气泡）。

```
        Agent 节点（宿主）                             操作者节点（GUI）
┌─────────────────────────────────────────┐        ┌──────────────────────────────────┐
│  apps/acp-agent（Rust 常驻桥，扩展）        │        │  apps/gui（Web）                   │
│  ├─ P2P Node                             │        │  ├─ /agents 页（消息页模式）        │
│  ├─ handler: /a2a/1 （新增）               │        │  │    发现/我的 双视图 + 创建对话框  │
│  │    card 相: 卡片查询/簿/推卡            │  QUIC  │  ├─ 聊天页 A2A 会话 kind（新增）     │
│  │    task 相: task 生命周期 + parts 流转  │  直连/  │  │    气泡渲染复用（parts→气泡）     │
│  │    桥接: task ⇄ 子进程 ACP session     │  打洞/  │  └─ 邀请帧处理（同意/拒绝）         │
│  ├─ 发布: rendezvous/mDNS（公开）          │ 加密中继│                                  │
│  ├─ 邀请帧投递（私有，复用转发模式）         │◄──────►│  apps/acp-console（扩展）           │
│  └─ dsh --profile acp 子进程（复用既有）    │ /a2a/1 │  ├─ 协议感知泵（携带协议 ID 拨号）   │
│                                          │  流    │  └─ agent 卡片发现事件             │
└─────────────────────────────────────────┘        └──────────────────────────────────┘
```

## 3. 组件清单（交付形态，非阶段）

| 组件 | 形态 | 职责 |
|---|---|---|
| `crates/a2a` | Rust lib（纯，零网络零进程） | AgentCard/SignedCard 模型、可见性、AgentBook（TTL 簿）、/a2a/1 帧编解码、task 状态机与 parts 类型、纯校验 |
| `apps/acp-agent` | Rust bin（扩展） | /a2a/1 handler（card 相 + task 相）、本地 agent 管理面（admin HTTP）、发布/推卡/邀请帧、task⇄ACP 桥接、授权门禁（私有清单） |
| `apps/acp-console` | Rust bin（扩展） | 携带协议 ID 的拨号泵（/dsh-acp/1 与 /a2a/1 均可）、agent 卡片发现事件透传（WS→GUI） |
| `apps/gui` | Web（扩展） | /agents 页（消息页模式）、创建/编辑对话框、A2A 会话 kind 聊天、邀请处理、路由/i18n 登记 |
| `apps/cli` p2pctl | 子命令（扩展） | headless 管理面：a2a list/publish/unpublish/allow（私有授权） |
| `crates/p2p-relay` proto_ids | 常量登记（加法） | /a2a/1 协议 ID 登记（底座唯一定义点，仅常量） |

分层纪律：A2A 业务不进底座 crates/p2p-*；`crates/a2a` 是纯应用库，宿主逻辑在 apps/。

## 4. AgentCard 模型（crates/a2a）

### 4.1 卡片字段（A2A 对齐 + p2p 扩展）

```json
{
  "agentId": "random-16-hex 或 owner 指定 slug",
  "name": "代码评审员",
  "description": "对 PR 做代码评审，给出改进建议",
  "url": "a2a://12D3KooW…abc/代码评审员",        // A2A url 字段 → p2p 寻址
  "hostPeer": "12D3KooW…abc",                    // 宿主 PeerId（= 签名 pubkey 绑定）
  "visibility": "public | private | local",      // p2p 扩展：可见性
  "capabilities": { "streaming": true, "pushNotifications": false },
  "skills": [
    { "id": "code-review", "name": "代码评审",
      "description": "评审代码并给出改进建议", "tags": ["review"] }
  ],
  "ttlSecs": 300,                                // 声明有效期（秒），过期即失效
  "version": 1                                    // 卡片版本（更新可被识别）
}
```

- **能力真相原则**：capabilities/skills 是宿主声明的自述；GUI 渲染时「未声明=不支持」灰化
  （对齐 capabilities-card.tsx 的 CapBadge 纪律，绝不代答 true/false）。
- 寻址 a2a://hostPeer/agentId 是**逻辑地址**；实际建连走 hostPeer 的地址簿/发现，
  agentId 由宿主在 /a2a/1 card 相解析（§5.1）。

### 4.2 签名信封（复用 llm-share-offer 模式）

```rust
pub struct SignedCard {
    pub card: AgentCard,
    pub issued_at: u64,          // unix 秒，入签（防旧卡重放）
    pub pubkey: [u8; 32],        // Ed25519，绑定 hostPeer
    pub sig: [u8; 64],           // canonical(card) + issued_at 小端 8 字节
}
```

- 验签纪律同 SignedOffer：peer 与 pubkey 绑定、签名、时间窗（now ∈ [issued_at, issued_at+ttl)）。
- 公开池条目 = 签名注册（rendezvous sign_register，namespace `a2a/agents/1`，TTL 即卡片 TTL）；
  卡片本体经 /a2a/1 card 相或建连推卡交换，订阅侧验签后入簿。

### 4.3 AgentBook（订阅侧）

同 OfferBook 模式：HashMap<agentKey, SignedCard>，insert 验签通过才入；live(now) 过滤 TTL；
evict_expired 留 WARN 观测。agentKey = (hostPeer, agentId) 复合键（同 host 可多 agent）。

## 5. /a2a/1 线协议

首帧 = 协议 ID（底座标准开手）。之后帧类型：`card`（card 相）与 `task`（task 相），
帧内 JSON（对齐 wire-protocol §3 JSON 编码）。双向同一条流。

### 5.1 card 相

| 方向 | 帧 | 语义 |
|---|---|---|
| C→S | { "v":1, "kind":"card", "op":"list" } | 拉取宿主全部卡片（验签后入簿） |
| C→S | { "v":1, "kind":"card", "op":"get", "agentId":"…" } | 单卡查询 |
| C→S | { "v":1, "kind":"card", "op":"subscribe" } | 订阅宿主卡片变更（推卡） |
| S→C | { "v":1, "kind":"card", "op":"cards", "cards":[SignedCard…] } | 批量应答 |
| S→C | { "v":1, "kind":"card", "op":"push", "cards":[SignedCard…] } | 变更推送（新增/更新/下架） |

下架帧：cards 数组携带 visibility=local 或 ttlSecs=0 的标记卡，订阅侧据此除名（§7.4）。

### 5.2 task 相（A2A Task 语义裁剪）

| 方向 | 帧 | 语义 |
|---|---|---|
| C→S | { "v":1, "kind":"task", "op":"create", "taskId":"<client 生成 uuid>", "agentId":"…", "message":{ "role":"user", "parts":[TextPart…] } } | 建 task 并发首条消息 |
| S→C | { "v":1, "kind":"task", "op":"status", "taskId":"…", "state":"submitted|working|completed|failed|cancelled|input_required|rejected" } | 状态推送 |
| S→C | { "v":1, "kind":"task", "op":"message", "taskId":"…", "message":{ "role":"agent", "parts":[…] } } | agent 消息（流式 = 多条 message 帧，parts 增量） |
| C→S | { "v":1, "kind":"task", "op":"send", "taskId":"…", "message":{…} } | 追加消息（续聊） |
| C→S | { "v":1, "kind":"task", "op":"get", "taskId":"…" } | 拉 task 快照（断线恢复） |
| C→S | { "v":1, "kind":"task", "op":"cancel", "taskId":"…" } | 取消（映射 ACP quiesce） |

- task 与 ACP session 1:1 映射：create/send → ACP session/new + prompt（追加消息 → 同 session 再 prompt）；
  message 帧 ← agent_message_chunk；status ← prompt 完成/失败/stopReason。
- 权限瀑布复用：request_permission 仍由桥代答或路由远程 GUI（§9 安全）。
- 无 id 的帧 = 通知（对齐 ACP notification 语义），id 帧请求-响应。

### 5.3 parts 类型（A2A 标准三件）

TextPart（`{"type":"text","text":"…"}`）、FilePart（`{"type":"file","file":{name,mimeType,bytes(base64)}}`）、
DataPart（`{"type":"data","data":{…}}`）。GUI 渲染：text → 气泡文本；file → 复用
media-content.tsx 附件渲染；data → 折叠信息卡。单帧 ≤ 1 MiB（底座约束），大文件走
chunked transfer（复用底座能力）。

## 6. 公开/私有语义

| 可见性 | 卡片进公共池 | 谁可发现 | 谁可聊 |
|---|---|---|---|
| public | 是（rendezvous + mDNS） | 全网节点 | 任意已认证 peer |
| private | 否 | 仅 owner + 被邀请且同意的 peer | 同上（授权清单） |
| local | 否 | 仅 owner 本机（loopback） | owner 本机 |

- 私有 agent 的卡片仍签名，但只点对点投递（邀请帧 §7.3）；接收方同意后入其 AgentBook，
  对方 hostPeer 的授权清单同步登记（双向确认，防冒名）。
- 授权清单存宿主数据目录（0600，复用 p2p-identity 种子落盘纪律）；owner 经 GUI/p2pctl 管理。

## 7. 发现与通知（「通过 p2p 协议通知其他节点」）

### 7.1 公开：rendezvous 命名空间 + mDNS

- 发布：宿主按卡片 TTL 周期签名注册到 namespace `a2a/agents/1`（复用 sign_register 公开设施，
  服务端 TTL 封顶 3600s，卡片 TTL 建议 300s 由宿主刷新）。
- 发现（拉）：订阅侧按现有退避逻辑（2×5s 快启动 → 30s±20% 稳态）拉 namespace 快照，
  得到「提供 agent 的 peer 集」，再经 card 相逐个取卡验签入簿。mDNS 局域网零配置同机制。
- 别名：namespace 按 peer 去重后卡片以 agentKey 入簿，天然支持同 peer 多 agent。

### 7.2 公开：已连 peer 推卡（建连即通知）

- 宿主与任意 peer 建连后（Node 事件 PeerConnected 触发），主动向对端发 card/subscribe，
  对端回 card/list 应答 → 双向即时同步卡片，不依赖轮询。
- 变更（发布/更新/下架）经 card/push 即时送达已订阅 peer。

### 7.3 私有：邀请帧（点对点通知）

- owner 发起：对目标 peer 发 `a2a-invite`（复用 relay 打洞信令的转发模式，目标不在线
  显式拒绝）；载荷 = 目标 peer 的 SignedCard + owner 签名。
- 接收方：卡片入「私密邀请」区（/agents 页），同意后入簿并回执（双向登记）。

### 7.4 下架与失效

- owner 下架：停发注册 + 向已订阅 peer 发 card/push（下架标记卡）→ 订阅侧除名。
- TTL 过期：AgentBook evict_expired 自动除名（WARN 观测）；不依赖显式下架。
- GUI 事件：node-event 加法字段 `agent_card`（cards 变更事件，契约纪律：只加字段不改形状）。

## 8. GUI 设计（消息页面模式，非下拉菜单）

对齐 docs/design/message-center 的既有模式：header + SegmentedControl + 分区列表 + 行内动作。

### 8.1 路由与登记

- 新 rail 入口「智能体」/agents（menu.def.ts append-only 注册，独立小提交；rail 7→8，
  Cmd/Ctrl+8）。i18n key：agents.title / agents.description 等（i18n types + locale 独立提交）。
- 路由 /agents 挂 AgentsPage；/chat 新增 A2A 会话 kind（SELECTION_KEYS 加 "a2a"）。

### 8.2 /agents 页结构（消息页同构）

```
header: 「智能体」标题 + 副标题「发布与发现 agent 智能体」 + SegmentedControl [发现 (n)] [我的]
─────────────────────────────────────────────────────
发现视图（默认）：
  ▸ 公开智能体 节（section-header: 图标 chip + 标题 + count 徽章）
     行（white card，10px 圆角，1px 灰 ring）:
       第一行: 头像图标(Bot) + 名称(medium) + 右侧行内动作 [聊天][详情]
       第二行: 描述截断(text-sm muted) + skills 徽章(tags)
       第三行: monospace 宿主 peer 短 id + copy 图标 + 在线点(绿/灰) + 可见性徽章
  ▸ 私密邀请 节（收到/发出 两向，对齐好友邀请模式）
     行: 发件 peer 名/短 id + agent 名 + 行内 [同意][拒绝] + 待处理徽章 + 时间
  ▸ 空态: EmptyState（"暂无公开智能体，可到「我的」创建发布"）
我的视图：
  ▸ 我发布的 节
     行: 名称 + 可见性徽章(公开=绿/私有=灰/local=灰) + 在线状态点 +
         行内动作 [聊天(自测)][编辑][分享(生成邀请)][下架]
  ▸ 创建智能体：节头右侧主按钮（+ 创建，消息页行内动作同款 Button）→ 创建对话框
```

- 创建对话框（对齐 endpoint-add-dialog 模式）：名称/描述/skills 多值输入/可见性
  SegmentedControl（公开/私有）/确认。校验失败原位上浮（focus-first-error 复用）。
- 行内动作反馈：AsyncButton + toast（复用反馈组件），失败原文上浮不静默。

### 8.3 聊天集成（直接跟已发现的 agent 聊天）

- 点「聊天」→ /chat?agent=<agentKey>（新 kind=a2a），右侧 AgentConversation 同构渲染：
  - 输入：A2A text part 发送（composer 复用）
  - 气泡：agent text parts 流式渲染（message-list/message-bubble 复用）
  - 状态徽章：task state（working=思考中 / completed=完成 / failed=失败）→ StatusBadge 复用
  - 附件：FilePart → media-content 复用；DataPart → 折叠信息卡
- 会话列表：useConversationEntries 扩展 a2a kind（入口来自 AgentBook），未读计数同源机制。
- 断线重连：task/get 拉快照续聊（对齐 ACP resume 语义，host 侧 task 状态驻留）。

### 8.4 徽标与入口

- rail 铃铛角标 = 消息中心 pending（既有）+ 私密邀请 in 向 pending（use-unread-total 加法）；
  /agents 页自身不设角标（浏览语义，对齐发现页）。

## 9. 安全模型

| 层 | 机制 |
|---|---|
| 认证 | 底座传输层互认 PeerId（QUIC TLS1.3 内嵌公钥 / Noise XX）——零成本密码学身份 |
| 卡片 | Ed25519 签名信封（peer 绑定 + 时间窗 + TTL），验签不过不入簿；公开池条目=签名注册防冒名 |
| 授权 | 私有 agent 授权清单（PeerId → 允许），默认拒绝；local 仅 loopback；owner 经 GUI/p2pctl 管理 |
| 宿主 | 复用 acp-agent：cwd 监狱（sandbox/workspace/owner 三级）、mcpServers 剥离/白名单、request_permission 瀑布（read/think/fetch=allow，execute/edit/delete=ask，60s 超时=reject） |
| 限流 | 每 peer 并发 task 上限（默认 4）、每卡片 RPM（默认 10，防公开池滥用）、连接数可配（对齐 acp-over-p2p §7 资源门禁） |
| 审计 | 复用 acp-agent 审计日志（conn-denied/gate-denied/task-denied/card-publish 等，target acp_audit） |
| 凭据 | API key 只在子进程环境；wire 上只有语义帧（对齐 ACP 哲学） |

**必须点破**：task 相把「远程发消息给 agent」变成第一类操作，等于把 acp-agent 的
「远程驱动一个握着工具的 agent」暴露给**任意已认证 peer**（公开 agent）。因此：
公开 agent 的 request_permission 一律走 owner 批准的交互面（超时拒绝），
禁止对公开 agent 放行 execute/edit/delete 的静态 allow（§9 授权行收紧为 ask 起步）。

## 10. 生命周期与故障矩阵

| 事件 | 宿主行为 | GUI 看到 |
|---|---|---|
| 卡片发布失败（rendezvous 不可达） | 重试退避 + WARN；本地可见性不受影响 | 我的页「发布失败」徽章 |
| task 建连失败（宿主离线） | - | 聊天页离线提示 + 重试 |
| 子进程 mid-turn 崩溃 | 断流 + 审计 | 失败气泡 + 重试 |
| TTL 过期 | 订阅侧自动除名 | 发现列表消失（可刷新） |
| 私有邀请目标不在线 | 转发失败显式拒绝 + 审计 | 发出侧显示「未送达」 |
| 非授权 peer 访问私有 agent | task 相 gate-denied 拒绝 + 审计 | （owner 日志可见） |

## 11. 分阶段计划与验收口径

### A2A1 crates/a2a 纯库

AgentCard/SignedCard 模型 + 可见性 + 校验 + 签名信封 + AgentBook + /a2a/1 帧编解码 +
task 状态机（parts 类型）。纯函数，单测全覆盖。
验收：`cargo test -p a2a && cargo clippy -p a2a -- -D warnings && make check`；
关键单测：验签失败三态（peer 绑定/签名/时间窗）、TTL 除名、帧编解码 roundtrip、
task 状态迁移非法路径拒绝。

### A2A2 acp-agent 宿主扩展

/a2a/1 handler（card 相 + task 相）、task⇄ACP session 桥接、本地 agent 管理面
（admin HTTP：create/update/remove + 可见性切换）、发布（rendezvous + mDNS）、
已连 peer 推卡（subscribe/push）、私有授权清单。
验收：双节点 itest——A 发布 public agent，B 经 rendezvous 发现 peer → card 相取卡验签入簿 →
task create/send → A 子进程产出 → message 帧回流 B；私有 agent 邀请→同意→聊全链；
非授权 peer 访问私有 agent 被 gate-denied。`make check` 全绿。

### A2A3 GUI /agents 页

/agents 路由 + rail 注册 + 发现/我的双视图 + 创建/编辑对话框 + 行内动作（消息页模式），
i18n + 单测（测试对齐 messages-page.test.tsx 结构）。
验收：`cd apps/gui && pnpm test --run src/routes/agents-*.test.tsx && pnpm build && pnpm check:i18n`；
渲染矩阵测试覆盖空态/行内动作/创建校验/可见性徽章。

### A2A4 GUI 聊天集成

A2A 会话 kind（SELECTION_KEYS + conversation-entry 扩展）、A2A 客户端（console 泵 +
帧编解码）、parts→气泡适配、task 状态徽章、断线 task/get 恢复。
验收：聊天矩阵测试（文本/附件/状态徽章/重连恢复）+ build + i18n 全绿。

### A2A5 私有邀请全链 + p2pctl

邀请帧端到端（GUI 发起 → 转发 → 接收方同意 → 双向登记）、rail 铃铛角标加法、
p2pctl a2a 子命令（list/publish/unpublish/allow）。
验收：双 GUI 节点邀请全链 itest + p2pctl 自测 + make check。

### A2A6 收尾

E2E 全链 itest 稳定化、文档（README/ops）、发布预检（make check + 构建产物）。
验收：make check 全绿 + 发布清单预检通过。

## 12. 风险

- **公开 agent 滥用**：任意 peer 可连 → 限流 + permission 收紧（§9 必须点破）+ 审计。
- **卡片伪造**：签名信封 + 时间窗 + TTL 三重防线；hostPeer 与 pubkey 绑定杜绝冒名。
- **TTL 与轮询延迟**：发现列表最多滞后一个 TTL（300s）；已连 peer 推卡覆盖即时场景。
- **A2A 语义裁剪边界**：streaming 以「多条 message 帧」实现，与 A2A 标准 streaming 兼容
  （parts 增量语义一致）；pushNotifications 不做（GUI 常在线，task/get 兜底）。
- **并行会话冲突**：迁移号/协议 ID 登记走既有占号纪律；本方案新增 crate 无迁移号冲突面。

## 13. 拍板项（定稿时裁决）

- Q1: 聊天传输走完整 /a2a/1 task 语义（推荐，真 A2A）还是复用 ACP 透传（省前端适配）？
- Q2: 私有邀请落 /agents 页「私密邀请」节（推荐，agent 事务集中）还是消息中心新增节？
- Q3: 公开 agent 的权限瀑布是否一律 ask（推荐）还是允许 owner 对指定 peer 静态 allow？
- Q4: 卡片 TTL 默认 300s（推荐）还是更短/更长？

