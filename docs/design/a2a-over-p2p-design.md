# A2A over P2P：Agent 智能体发布 / 发现 / 直连聊天 —— 设计方案 v1（定稿）

> 状态: v1 定稿 | 日期: 2026-09-08
> 审查: 四路 subagent REVISE 已合并（架构 20 / 安全 10 / GUI 12 / 范围 14 条），
> 痕迹留档 docs/notes/2026-09-08-a2a-review-closeout.md；拍板记录 §14。
> 依赖: [p2p-base-design.md](p2p-base-design.md)、[wire-protocol.md](wire-protocol.md)、
> [acp-over-p2p-design.md](acp-over-p2p-design.md)、[idle-token-sharing-plan.md](idle-token-sharing-plan.md)、
> [social-discovery-plan.md](social-discovery-plan.md)（§P2 签名凭证帧先例）
> 参照: Google A2A 协议（a2a-protocol.org，Agent Card / Task / Message-Part / JSON-RPC 2.0）

把 Google A2A（Agent2Agent）协议的能力声明与任务语义嫁接到 p2p-base 底座：
节点创建**公开/私有** agent 智能体，经 P2P 通知其他节点，其他节点**发现 agent 能力**后
**直接与之聊天**。聊天走 A2A 任务语义（JSON-RPC 2.0 task + message/part），宿主复用
acp-agent 的 dsh 子进程托管资产（监狱/mcpServers 改写/权限瀑布/slot 模型）。

## 1. 背景与目标

### 1.1 A2A 协议要点（本方案采纳的子集）

| A2A 概念 | 语义 | 本方案落点 |
|---|---|---|
| Agent Card | agent 能力自述：name/description/url/capabilities/skills/securitySchemes | 卡片模型 + 签名信封（§4） |
| Task | 工作单元，生命周期 submitted→working→completed/failed/cancelled/input_required/auth_required/rejected | /a2a/1 task 相（§5.2，JSON-RPC 2.0） |
| Message/Part | role(user/agent) + parts：TextPart/FilePart/DataPart | task 内消息体（§5.3） |
| 发现 | 拉取 Agent Card（标准是 HTTP well-known/registry） | rendezvous 命名空间 + mDNS + 已连 peer 推卡 + 心跳刷新（§7） |
| 传输 | JSON-RPC 2.0 over HTTP | p2p 协议流（QUIC/TCP，varint 帧）替代 HTTP |

A2A 标准的 url 字段落成 **a2a://<hostPeerId>/<agentId>**；securitySchemes 由底座传输层
互认身份替代（不另发明 OAuth）。

### 1.2 目标

1. 节点可创建 agent 智能体，声明**公开或私有**可见性。
2. 公开 agent 经 P2P 通知全网可发现；私有 agent 经签名凭证邀请帧点对点通知指定节点。
3. 其他节点发现 agent 的**能力**（skills/capabilities）后可一键**直连聊天**。
4. 聊天走 A2A 任务语义（真 A2A，非 ACP 透传），宿主复用 acp-agent 的 dsh 子进程托管。
5. p2p-base 底座一层不侵：A2A 是底座的应用（协议 ID 自 crate 定义 + 文档登记）。

### 1.3 非目标

- 不做 agent 间自动协商/编排（那是 E10 llm-share 与后续 agent 编排的课题）。
- 不做 A2A 全量规范（pushNotifications/artifact 流转不做；thoughts/tools 负空间见 §5.2）。
- 不做 agent 市场/评分/信誉体系。
- 不承诺发现「全部」agent；发现收敛为「找到并聊上可用的 agent」（social-discovery-plan 语义）。

## 2. 立场与总架构

**一句话架构**：Rust 纯库 `crates/a2a`（零网络零进程）定义 AgentCard/签名信封/AgentBook/
帧编解码/task 状态机；宿主 `apps/acp-agent` 挂 /a2a/1 handler——card 相 + task 相，
task 相内嵌 ACP client 把 A2A 任务桥接到既有 dsh 子进程的 ACP session；GUI 新增
`/agents` 页（消息页面模式）与 A2A 会话（`?a2a=` 新 query），A2A 记录区复用
message-list/message-bubble 渲染 parts。

**关键映射（拍板 Q5/Q10）**：
- 1 task = 1 条 P2P 流 = 1 个 dsh 子进程（复用 acp-agent slot 模型，进程边界=任务边界，
  对齐 acp-over-p2p「进程边界=连接边界」纪律）；
- task⇄ACP：create/send → ACP session/new + prompt；message 帧 ← agent_message_chunk；
  status ← stopReason/prompt 结束；cancel → 该流子进程 quiesce（EOF→宽限→SIGKILL）+
  status cancelled + 审计；
- v1 只支持 TextPart 上行；FilePart/DataPart 接收渲染为附件卡/折叠卡，上行显式拒绝
  （error 帧 file-parts-unsupported，不静默）。

```
        Agent 节点（宿主）                             操作者节点（GUI）
┌─────────────────────────────────────────┐        ┌──────────────────────────────────┐
│  apps/acp-agent（Rust 常驻桥，扩展）        │        │  apps/gui（Web）                   │
│  ├─ P2P Node                             │        │  ├─ /agents 页（消息页模式）        │
│  ├─ handler: /a2a/1                      │        │  │    发现/我的 双视图 + 创建对话框  │
│  │    card 相: list/get/subscribe/push   │  QUIC  │  ├─ 聊天页 A2A 会话 ?a2a=（新增）     │
│  │    task 相: JSON-RPC 2.0（1 task=1 流）│  直连/  │  │    A2aConversation（气泡栈复用）  │
│  │    桥: task ⇄ ACP session（内嵌 client）│  打洞/  │  └─ 邀请帧处理（同意/拒绝）         │
│  ├─ 发布: rendezvous 心跳 + mDNS（公开）   │ 加密中继│                                  │
│  ├─ 邀请: 签名凭证帧（私有）               │◄──────►│  apps/acp-console（扩展）           │
│  └─ dsh --profile acp 子进程（slot 复用）  │ /a2a/1 │  ├─ ?proto= 协议感知拨号            │
│                                          │  流    │  └─ 卡片/邀请事件 WS 通道（新增）    │
└─────────────────────────────────────────┘        └──────────────────────────────────┘
```

## 3. 组件清单（交付形态，非阶段）

| 组件 | 形态 | 职责 |
|---|---|---|
| `crates/p2p-identity` signed 模块 | Rust lib（纯加法） | 泛型签名信封 `Signed<T>`：b58 serde / canonical payload / VerifyError / 时间窗校验（llm-share-offer 内部委托，wire 零变更） |
| `crates/a2a` | Rust lib（纯，零网络零进程；依赖仅 serde/serde_json/bs58/ed25519） | AgentCard/SignedCard、可见性、AgentBook（TTL 钳制+容量+版本替换）、/a2a/1 帧编解码（card 相 + JSON-RPC 2.0 task 相）、task 状态机、parts 类型、PROTOCOL_ID 常量 |
| `apps/acp-agent` | Rust bin（扩展） | /a2a/1 handler（card 相 + task 相）、本地 agent 管理 admin HTTP、发布/推卡/心跳、签名凭证邀请、task⇄ACP 桥（内嵌 ACP client）、授权清单门禁、可见性权限维度 |
| `apps/acp-console` | Rust bin（扩展） | ?proto 协议感知拨号（/dsh-acp/1 与 /a2a/1）、卡片/邀请事件 WS 通道（独立于 node-event） |
| `apps/gui` | Web（扩展） | /agents 页、创建/编辑对话框（skills chip 输入）、A2A 会话 kind、邀请处理、路由/i18n/事件登记 |
| `apps/cli` p2pctl | 子命令（扩展） | a2a list/publish/unpublish/allow（headless 管理面） |
| 文档 | wire-protocol.md / gui-contract.md / cli-parity.tsv / p2pctl-ai-guide.md | /a2a/1 登记、console WS 契约、命令面登记（随对应阶段独立小提交） |

分层纪律：A2A 业务不进底座 crates/p2p-*（p2p-identity::signed 是纯加法工具模块，
非内核语义变更）；协议 ID 自 `crates/a2a` 定义（llm-share-offer 先例），
wire-protocol §3.2 表登记一行。

## 4. AgentCard 模型（crates/a2a）

### 4.1 卡片字段（A2A 对齐 + p2p 扩展）

```json
{
  "agentId": "kebab-slug（仅 [a-z0-9-]，≤32 字符）",
  "name": "代码评审员",
  "description": "对 PR 做代码评审，给出改进建议",
  "url": "a2a://12D3KooW…abc/code-review",
  "hostPeer": "12D3KooW…abc",
  "visibility": "public | private | local",
  "capabilities": { "streaming": true },
  "skills": [ { "id": "code-review", "name": "代码评审", "description": "…", "tags": ["review"] } ],
  "ttlSecs": 300,
  "version": 1
}
```

- serde `deny_unknown_fields`：未知字段拒收（防协议漂移）。
- 能力真相原则：capabilities/skills 是宿主自述；GUI「未声明=不支持」灰化（CapBadge 纪律）。
- url 为逻辑地址；建连走 hostPeer 地址簿/发现，agentId 由宿主 card 相解析。

### 4.2 签名信封（复用，非重写）

```rust
// crates/p2p-identity::signed（纯加法）
pub struct Signed<T> {
    pub payload: T,        // serde 字段名 payload（新信封）；llm-share-offer 保持 offer 字段名
    pub issued_at: u64,
    pub pubkey: [u8; 32],  // b58
    pub sig: [u8; 64],     // b58
}
pub enum VerifyError { PeerMismatch, BadSignature, NotYetValid, Expired(u64), Encoding }
impl<T: Serialize> Signed<T> {
    pub fn sign(payload: &T, kp: &Keypair, issued_at: u64) -> Result<Self, VerifyError>;
    pub fn verify(&self, now: u64) -> Result<(), VerifyError>;  // 时间窗 [issued_at, issued_at+ttl)
    pub fn payload_id(&self, id_fn: fn(&T) -> Option<PeerId>) -> Option<PeerId>;  // 绑定校验
}
```

- 纯加法模块 + llm-share-offer 内部委托共享助手（`signed` 的 b58/时间窗/签名原语），
  wire 格式零变更（llm-share-offer 已进 main，wire 是契约）。
- SignedCard = Signed<AgentCard> + hostPeer 绑定校验（pubkey 推导 PeerId == card.hostPeer）。

### 4.3 AgentBook（订阅侧）

- 键 (hostPeer, agentId) 复合；insert 验签 + **钳制**：TTL≤3600、issued_at 偏差≤300s、
  容量上限（默认 256 条，超限逐最旧）；**版本替换**：同键 version 升序才覆盖。
- live(now) 过滤 TTL；evict_expired 留 WARN；心跳刷新见 §7.1（订阅簿寿命 ≠ 发现 TTL）。

## 5. /a2a/1 线协议

首帧 = 协议 ID（底座标准开手，payload 即 `/a2a/1` UTF-8）。之后帧 JSON，双向同流。
card 相与 task 相**各自独立开流**（card 相是短连接请求-响应；task 相一条流承载一个任务）。

### 5.1 card 相（请求-响应，含 id 关联）

| 方向 | 帧 | 语义 |
|---|---|---|
| C→S | { "v":1, "id":1, "op":"list" } | 拉取宿主全部**对请求方可见**卡片（public 全集 + private 授权内） |
| C→S | { "v":1, "id":2, "op":"get", "agentId":"…" } | 单卡查询（不可见 → error 帧 not-found） |
| C→S | { "v":1, "id":3, "op":"subscribe" } | 订阅变更；应答含当前快照 |
| S→C | { "v":1, "id":1, "op":"cards", "cards":[SignedCard…] } | 应答（id 回显） |
| S→C | { "v":1, "id":3, "op":"ok", "subscribed":true } | subscribe 应答 |
| S→C | { "v":1, "id":0, "op":"push", "cards":[…], "removed":[agentKey…] } | 变更推送（新增/更新/移除，id=0 即通知） |
| S→C | { "v":1, "id":N, "op":"error", "code":"not-found|denied|bad-card", "message":"…" } | 错误应答（id 回显） |

- 可见性过滤（F4/F6）：list/push 只含请求方 PeerId 可见的卡（public 全部；private 仅授权
  清单内）；无公开卡且无私有授权的节点对 list 回空卡集。
- 移除用独立 **card/remove 帧**（§7.4），不用 ttlSecs=0 墓碑（过不了验签时间窗）。
- 重复 subscribe 幂等（同 peer 重复订阅返回当前快照，不重复推送登记）。

### 5.2 task 相（JSON-RPC 2.0，1 task = 1 流）

| 方向 | 帧 | 语义 |
|---|---|---|
| C→S | { "jsonrpc":"2.0", "id":1, "method":"tasks/create", "params":{ "agentId":"…", "message":{ "role":"user", "parts":[TextPart] } } } | 建 task 并发首条消息（服务端生成 taskId，返回 result 含 taskId） |
| S→C | { "jsonrpc":"2.0", "method":"tasks/status", "params":{ "taskId":"…", "state":"submitted|working|completed|failed|cancelled|rejected" } } | 状态通知（无 id） |
| S→C | { "jsonrpc":"2.0", "method":"tasks/message", "params":{ "taskId":"…", "messageId":"uuid", "message":{ "role":"agent", "parts":[…] } } } | agent 消息（messageId 客户端去重；流式 = 多帧同 taskId，快照合并见下） |
| C→S | { "jsonrpc":"2.0", "id":2, "method":"tasks/send", "params":{ "taskId":"…", "message":{…} } } | 追加消息（续聊） |
| C→S | { "jsonrpc":"2.0", "id":3, "method":"tasks/get", "params":{ "taskId":"…" } } | 拉 task 快照（断线恢复）：result = { state, messages:[…] }（服务端驻留按 taskId） |
| C→S | { "jsonrpc":"2.0", "id":4, "method":"tasks/cancel", "params":{ "taskId":"…" } } | 取消：桥对该流子进程 quiesce + status cancelled + 审计 |
| S→C | { "jsonrpc":"2.0", "id":N, "result":{…} } / { "jsonrpc":"2.0", "id":N, "error":{ "code":-32602, "message":"…" } } | 应答/错误（JSON-RPC 2.0 标准 + A2A 业务码） |

- **流式合并**：tasks/message 多帧同 taskId 时，客户端按 messageId 去重、同 messageId 的
  text parts 按到达序拼接（增量语义）；tasks/get 快照是权威终态，重连后以快照为准覆盖。
- **负空间（v1 声明）**：thoughts/tool_call 不进 wire（桥内执行，仅状态可见）；
  input_required/auth_required 状态 v1 不产生（权限瀑布在桥内闭环，见 §9）；
  rejected = 权限拒绝或门禁拒绝。
- 资源门禁：每 peer 并发 task 流 ≤4；每流 FilePart 接收 ≤4 MiB（单帧 ≤1 MiB 底座约束，
  走 chunked transfer 重组）；每 task 累计上行输入 ≤256 KiB；子进程总量可配。

### 5.3 parts 类型（A2A 标准三件）

TextPart / FilePart（bytes base64 ≤4 MiB）/ DataPart。v1 上行仅 TextPart；FilePart/DataPart
仅接收渲染（附件卡/折叠信息卡），上行显式 error 拒绝。

## 6. 公开/私有语义

| 可见性 | 卡片进公共池 | 谁可发现 | 谁可聊 | 权限路由 |
|---|---|---|---|---|
| public | 是（rendezvous 心跳 + mDNS） | 全网节点 | 任意已认证 peer | think=桥代答；read/fetch/execute/edit/delete=ask OwnerLocal（§9 矩阵） |
| private | 否 | 仅 owner + 已同意邀请的 peer | 授权清单内 | 同 public（ask OwnerLocal），清单外 gate-denied |
| local | 否 | 仅 owner（loopback） | owner | 同 ACP owner scope 全权 |

- 私有 agent 卡片仍签名，只经**签名凭证邀请帧**点对点投递（§7.3，nonce 一次性/过期/
  invitee 绑定/回执签名，对齐 social-discovery-plan §P2）。
- 授权清单存宿主数据目录（0600，复用 p2p-identity 落盘纪律）；**撤销传播**：owner 移除
  授权 → card/remove 帧推给已同意 peer（§7.4，F20）。

## 7. 发现与通知（「通过 p2p 协议通知其他节点」）

### 7.1 公开：rendezvous 心跳 + mDNS

- 发布：宿主按卡 TTL（默认 300s）周期签名注册 namespace `a2a/agents/1`（sign_register
  公开设施，服务端 TTL 封顶 3600s）。**心跳**：每 TTL/2 发一次 card/push 到已订阅 peer，
  订阅簿寿命与发现 TTL 解耦——**2×TTL 无心跳才除名**（防 agent 周期性消失 flap，F5）。
- 发现（拉）：订阅侧退避拉 namespace 快照（2×5s 快启动 → 30s±20% 稳态）→ 得「提供 agent
  的 peer 集」→ card 相逐个取卡验签入簿；mDNS 局域网同机制（单测覆盖，不进 itest 验收）。
- 无公开卡的节点不注册进命名空间（F6）；公开池规模认知 512 上限（§12 风险）。

### 7.2 公开：已连 peer 推卡（建连即通知）

- PeerConnected 触发双向 card/subscribe ↔ list 应答，即时同步不依赖轮询；变更经
  card/push（含 removed 数组）即时送达。

### 7.3 私有：签名凭证邀请帧（点对点通知）

- wire 对齐 /im/invite/1 纪律 + social-discovery §P2：载荷 = SignedCard + nonce（一次性，
  宿主持久化防重放）+ expiry（默认 24h）+ invitee PeerId 绑定；owner 签名。
- 投递：复用 relay 打洞信令转发模式；目标不在线显式拒绝（「未送达」可观测）。
- 接收方同意 → 回执 = invitee 签名(nonce, agentId, hostPeer) → 双方登记（授权清单 +
  AgentBook）；拒绝 → 回执拒绝，nonce 作废。

### 7.4 下架/移除/失效

- 下架：停发心跳 + 向已订阅 peer 发 card/remove 帧（removed 数组）→ 订阅侧除名。
- 失效：2×TTL 无心跳，订阅侧 evict（WARN）。
- 事件通道（归一，F6/F11）：卡片/邀请变更事件走 **acp-console WS 独立事件通道**
  （与 ACP 事件管线同源），node-event 判别联合（ipc-types.ts NodeEventJson）不动。

## 8. GUI 设计（消息页面模式，非下拉菜单）

对齐 docs/design/message-center 既有模式：header + SegmentedControl + 分区列表 + 行内动作。

### 8.1 路由与登记

- rail 新增「智能体」/agents（menu.def.ts append-only 独立提交；位置=llm-share 后、
  settings 前；lucide Sparkles；Cmd/Ctrl+8；use-hotkeys 上限 9 吻合）。同提交修正
  App.tsx「rail 保持 4 项」过期注释。
- 中央登记：menu.def.ts / App.tsx 路由树 / i18n types+locale 各独立小提交；
  /agents 挂 AgentsPage（src/views/agents/agents-page.tsx）。
- 聊天路由：新增独立 query 键 **?a2a=**（SELECTION_KEYS 加 "a2a"），**不塞 ?agent=**
  （避免与 ACP endpointId 焦点冲突）。

### 8.2 /agents 页结构（消息页同构）

```
header: 「智能体」标题 + 副标题「发布与发现 agent 智能体」 + SegmentedControl [发现 (n)] [我的]
─────────────────────────────────────────────────────
发现视图（默认）：
  ▸ 公开智能体 节（共享 SectionHeader: 图标 chip + 标题 + count 徽章）
     行（bg-card ring-1 ring-border rounded-lg，沿用既有 token）:
       第一行: 头像(Bot) + 名称(medium) + 右侧行内动作 [聊天][详情]
       第二行: 描述截断(text-sm muted) + skills 徽章
       第三行: monospace 宿主 peer 短 id + copy 图标 + 在线点(green/yellow/red/gray) + 可见性徽章
  ▸ 私密邀请 节（收到/发出 两向，对齐好友邀请模式；同意免备注输入）
     行: 发件 peer 名/短 id + agent 名 + 行内 [同意][拒绝] + 待处理徽章 + 时间
  ▸ 空态: EmptyState（"暂无公开智能体，可到「我的」创建发布"）
我的视图：
  ▸ 我发布的 节
     行: 名称 + 可见性徽章(公开=绿/私有=灰/local=灰) + 在线状态点 +
         行内动作 [聊天(自测)][编辑][分享(生成邀请)][下架]
  ▸ 创建智能体：节头右侧主按钮（+ 创建）→ 创建对话框
```

- **SectionHeader 泛化**（F8）：messages/section-header.tsx 提为共享组件（views/shared/
  section-header.tsx，tone 扩 primary|info|agents），消息页与 /agents 页共用，禁双源。
- 创建对话框（endpoint-add-dialog 模式）：名称/描述/skills **chip 输入**（新建小组件，
  ≤10 条，trim+去重）/可见性 SegmentedControl（公开/私有）/确认；校验失败原位上浮。
- 行内动作反馈：AsyncButton + toast，失败原文上浮不静默；「分享」= 生成签名凭证邀请
  （targets picker 复用 entity-combobox，对齐 group-invite-dialog 先例）。

### 8.3 聊天集成（直接跟已发现的 agent 聊天）

- 点「聊天」→ /chat?a2a=<agentKey>，右侧 **A2aConversation**（新组件，chat-page.tsx
  不加厚）：
  - 记录区：复用 message-list/message-bubble（parts→气泡天然适配；消息形态而非
    ACP transcript 栈，与 ACP AgentConversation 分家）；
  - 输入：composer 复用，发 TextPart；
  - 状态徽章：task state → StatusBadge（working=思考中/completed=完成/failed=失败）；
  - 附件：agent FilePart → media-content 复用；DataPart → 折叠信息卡；
  - 断线重连：tasks/get 拉快照续聊（messageId 去重 + 快照覆盖）。
- 会话列表：ConversationKind 加 "a2a"，**触碰面全列**（A2A4）：
  SELECTION_KEYS / conversation-prefs-store conversationKey 键空间 / conversation-row
  kindMark 分支 / chat-page selectEntry kind→query 映射与 ?kind= 分支 / ChatEmptyState
  计数 / use-unread-total 求和 / conversation-entry 构建器。**拆文件**
  conversation-entry-a2a.ts（conversation-entry.ts 已 278 行，红线 300）。

### 8.4 徽标与入口

- rail 铃铛角标 = 消息中心 pending（既有 selectPendingInviteBadgeCount）+ 私密邀请 in 向
  pending（并入同一 selector，非 use-unread-total）；/agents 页自身不设角标。

## 9. 安全模型（远程驱动握着工具的 agent——生死线）

**权限路由矩阵（拍板 Q3，写死）：**

| 可见性 | scope | think | read/fetch | execute/edit/delete | ask 路由 |
|---|---|---|---|---|---|
| public | sandbox（强制） | 桥代答 allow | **ask**（不静态放行） | ask | **OwnerLocal**（本地审计 + reject-once 占位 + owner 批准面） |
| private | sandbox（强制） | 桥代答 allow | ask | ask | OwnerLocal |
| local | owner 全权（loopback） | 桥代答 allow | 静态 allow | ask | RemoteGui（owner 本机 GUI） |

- **绝不**对 public/private 远程 agent 用 RemoteGui 路由（陌生人批准 execute = 宿主 RCE）。
- 公开 agent 一律 ask 起步；owner 不在场即超时 reject（60s），agent 无法越权。
- 其他闸（安全审查 F1-F7 全采纳）：
  - 卡片签名信封（peer 绑定 + 时间窗 + TTL 钳制）；公开池条目=签名注册防冒名；
  - AgentBook 验签 + 钳制（§4.3）；card 相按请求方过滤可见性；
  - 每 peer 独立子进程（slot 模型复用），task 按创建者 PeerId 鉴权；宿主级并发/RPM/子进程
    总量；FilePart ≤4 MiB；每 task 输入累计上限；配额审计；
  - 邀请=签名凭证帧（nonce/expiry/invitee 绑定/回执签名），防放大授权；
  - 复用 acp-agent 审计（新增 card-publish / card-remove / invite-denied / task-denied /
    task-cancelled 事件）；API key 只在子进程环境。

## 10. 生命周期与故障矩阵

| 事件 | 宿主行为 | GUI 看到 |
|---|---|---|
| 卡片发布失败（rendezvous 不可达） | 重试退避 + WARN；本地可见性不受影响 | 我的页「发布失败」徽章 |
| 心跳超时（2×TTL） | 订阅侧除名 | 发现列表消失（刷新可重拉） |
| task 建连失败（宿主离线） | - | 聊天页离线提示 + 重试 |
| 子进程 mid-turn 崩溃 | 断流 + 审计 | 失败气泡 + 重试 |
| 私有邀请目标不在线 | 转发失败显式拒绝 + 审计 | 发出侧「未送达」 |
| 非授权 peer 访问私有 agent | task 相 gate-denied + 审计 | （owner 日志可见） |
| 权限 ask 超时 | reject-once | 气泡显示「已拒绝（超时）」 |

## 11. 分阶段计划与验收口径

> 每阶段验收命令必须真实可执行；acp-agent/acp-console 是独立 cargo 项目（根 Cargo.toml
> exclude），**make check 不覆盖其单测**，验收显式补其测试域。单文件 ≤300 行红线。

### A2A1 crates/a2a 纯库 + p2p-identity::signed（~1000 行，文件级拆分）

- p2p-identity::signed 泛型信封（Signed<T>/VerifyError/b58/时间窗）+ llm-share-offer
  内部委托（wire 零变更，其测试保持绿）；
- crates/a2a：AgentCard/SignedCard/可见性校验/AgentBook（钳制+容量+版本替换）/card 相帧/
  JSON-RPC 2.0 task 相帧/parts 类型/task 状态机/PROTOCOL_ID。
- 新增文件：crates/a2a/src/{lib,card,book,frame,task,proto}.rs（各 ≤300 行）+ 单测；
  crates/p2p-identity/src/signed.rs。
- 验收：`cargo test -p a2a && cargo test -p p2p-identity && cargo test -p llm-share-offer &&
  cargo clippy -p a2a -p p2p-identity -p llm-share-offer -- -D warnings && make check`；
  关键单测：验签三态、TTL 钳制（>3600 拒）、issued_at 偏差（>300s 拒）、版本替换、
  帧 roundtrip、task 状态迁移非法路径拒绝、JSON-RPC 错误帧。

### A2A2a acp-agent card 相（~900 行）

- /a2a/1 card 相 handler（list/get/subscribe/push/remove，按请求方可见性过滤）、
  本地 agent 管理 admin HTTP（create/update/remove/visibility）、rendezvous 心跳发布、
  已连 peer 推卡、wire-protocol §3.2 登记 /a2a/1 行。
- 验收：`cd apps/acp-agent && cargo test && cargo clippy --all-targets -- -D warnings` +
  p2p-itest card 链（双节点：A 发布 public agent → B rendezvous 发现 peer → card/list 取卡
  验签入簿 → push 心跳 → card/remove 除名；默认 acp-echo-stub 子进程，真 dsh #[ignore]）。
  `make check` 全绿。

### A2A2b acp-agent task 相（~1000 行）

- task 相接线（1 task=1 流）、task⇄ACP 桥（内嵌 ACP client：create/send/get/cancel ↔
  session/prompt/status）、per-peer 子进程（slot 复用）、私有授权清单门禁 + 邀请回执登记、
  可见性权限维度（permission.rs 加 visibility 分支 + 回归）、限流（并发/FilePart/累计输入）。
- 验收：acp-agent 测试域 + p2p-itest task 链（task create/send → stub 产出 → message 帧
  回流 → status 终态 → cancel → cancelled；非授权 peer 访问私有 agent gate-denied；
  权限 ask 超时 reject-once）。`make check` 全绿。

### A2A3 GUI /agents 页 + console 扩展（~1200 行）

- /agents 路由 + rail 注册（Sparkles，Cmd+8，修正过期注释）+ 发现/我的双视图 +
  创建/编辑对话框（skills chip 输入）+ 行内动作 + SectionHeader 泛化 + 空态 + i18n；
  console ?proto 拨号 + 卡片/邀请事件 WS 通道（gui-contract.md 契约加法同步）。
- 验收：`cd apps/gui && pnpm test --run src/views/agents && pnpm build && pnpm check:i18n`
  + 渲染矩阵测试（空态/行内动作/创建校验/可见性徽章/邀请行）；acp-console 测试域。

### A2A4 GUI A2A 聊天（~1000 行）

- ?a2a= query + SELECTION_KEYS + conversation-entry-a2a.ts（拆文件）+ A2aConversation
  （message-list/message-bubble 复用，parts→气泡、task 状态徽章、附件卡、折叠卡）+
  tasks/get 断线恢复 + 触碰面全量（conversationKey/kindMark/selectEntry/EmptyState/unread）。
- 验收：聊天矩阵测试（文本流式/附件渲染/状态徽章/重连恢复/上行 FilePart 拒绝提示）+
  build + i18n 全绿。

### A2A5 私有邀请全链 + p2pctl（~900 行）

- 邀请帧 wire（nonce/expiry/invitee 绑定/回执签名）+ GUI 私密邀请节（同意/拒绝）+ 铃铛
  角标（selectPendingInviteBadgeCount 并入）+ p2pctl a2a 子命令（list/publish/unpublish/
  allow）+ cli-parity.tsv 加行 + p2pctl-ai-guide.md 同步。
- 验收：p2p-itest 邀请全链（agent 级双节点：owner 发起 → 接收方同意 → 双向登记 → 授权
  内可聊 → 撤销传播）+ GUI 组件测试 + p2pctl 自测 + make check。

### A2A6 收尾

- E2E itest 稳定化（stub 默认 + 真 dsh #[ignore] + SKIP 信号）、README/ops 文档、
  gui-contract §17 契约收口、发布预检。
- 验收：make check 全绿 + 发布清单预检通过。

**分阶段合并纪律**：每阶段完成即合并 main（短命分支，不过夜）；A2A2a/b 动工前先
`git fetch origin && git merge main` 确认 acp-local-loop/ux3-diag 合入状态。

## 12. 风险

- **公开 agent 滥用**：限流（§5.2）+ 权限矩阵（§9）+ 审计；公开池 512 上限认知。
- **卡片伪造/重放**：签名信封 + 时间窗 + TTL 钳制 + nonce；hostPeer↔pubkey 绑定。
- **发现延迟**：发现列表最多滞后一个 TTL；心跳 + 已连推卡覆盖即时场景。
- **A2A 语义裁剪边界**：v1 只 TextPart 上行、无 thoughts/tools wire（负空间显式声明）。
- **并行冲突**：acp-local-loop 与 A2A2 文件域重叠 → 反向同步纪律；i18n/menu 撞号 →
  独立小提交 + 反向同步后定号。

## 13. 与仓库纪律对照

| 纪律 | 落点 |
|---|---|
| 中央登记 append-only | menu.def.ts / App.tsx / i18n types+locale 各独立小提交；wire-protocol §3.2、gui-contract §17、cli-parity.tsv、p2pctl-ai-guide.md 随阶段 |
| 能复用不要早轮子 | p2p-identity::signed 泛型信封（llm-share-offer 委托）；SectionHeader 泛化；message-list 复用 |
| 行数红线 300 | crates/a2a 文件级拆分；conversation-entry-a2a.ts 拆文件；A2aConversation 独立组件 |
| 占号/撞号 | 协议 ID /a2a/1 无冲突（已核验两在途分支未触碰 proto_ids）；i18n keys 反向同步后定号 |
| 短命分支 | 每阶段当天合并 main |

## 14. 拍板记录（定稿裁决）

| 项 | 裁决 | 理由 |
|---|---|---|
| Q1 聊天传输 | **真 A2A task 语义**（JSON-RPC 2.0 帧形） | 选透传则 Task 生命周期承诺落空，「A2A over P2P」名不副实；增量集中在纯库+桥+GUI kind，可控 |
| Q2 私有邀请落点 | **/agents 页「私密邀请」节** | 与好友邀请模式同构，agent 事务集中 |
| Q3 公开 agent 权限 | **一律 ask 起步，OwnerLocal 路由**（绝不 RemoteGui） | 陌生人批准 execute=RCE；owner 不在场超时拒绝 |
| Q4 卡片 TTL | **300s 发现 TTL + 心跳刷新（TTL/2），2×TTL 无心跳除名** | 防 flap；与服务端封顶兼容 |
| Q5 A2A 记录区渲染栈 | **新 A2aConversation 复用 message-list/message-bubble** | parts 是消息形态，天然适配气泡栈；与 ACP transcript 栈分家 |
| Q6 卡片事件通道 | **acp-console WS 独立事件通道** | node-event 判别联合改成员=形状变更，不动 |
| Q7 /agents rail 位置图标 | **llm-share 后、settings 前；Sparkles；Cmd/Ctrl+8** | 与 use-hotkeys 上限 9 吻合 |
| Q8 私有邀请同意语义 | **免备注输入** | agent 邀请按 agent 事务，非好友关系 |
| Q9 skills 输入 | **chip 输入（新建小组件，≤10 条，trim+去重）** | 仓库无现成 tag 多值输入 |
| Q10 task/流/子进程映射 | **1 task = 1 流 = 1 子进程**；cancel=子进程 quiesce | 对齐「进程边界=连接边界」；免 per-prompt cancel 复杂度 |
| Q11 FilePart 上行 | **v1 拒绝（显式 error），仅接收渲染** | 免 sandbox 注入机制复杂度，显式不静默 |
| Q12 协议 ID 定义点 | **crates/a2a 自定义 + wire-protocol §3.2 登记** | llm-share-offer 先例；业务协议自 crate 定义 |

