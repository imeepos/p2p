# /a2a/1 规范

状态：stable；自 2026-09-07；归属 crates/a2a；符合性：Core=card 相帧表与签名卡片、task 相 JSON-RPC 2.0 与状态机、可见性 fail-closed 门禁；Extended=签名凭证邀请四帧、私有 agent 授权清单。

## 1. 概览

A2A over P2P：节点发布/发现 agent 智能体并与之直接任务对话。协议分两相，各自独立开流，一条流内不分相：card 相（请求-响应 + 订阅推送，承载卡片发现与邀请）与 task 相（JSON-RPC 2.0，一条流承载一个任务，1 task = 1 流 = 1 宿主子进程）。

与 Google A2A（a2a-protocol.org）的继承/裁剪边界（逐条）：

| 项 | 处置 | 说明 |
|---|---|---|
| Agent Card 概念与 name/description/url/capabilities/skills 字段 | 继承 | 字段语义对齐，wire 名 camelCase |
| url 字段 | 改义 | 逻辑地址 `a2a://<hostPeer>/<agentId>`，建连走底座发现，不做 HTTP well-known |
| securitySchemes/OAuth | 裁剪 | 底座 Noise 互认身份 + PeerId 替代，不另发明鉴权 |
| Task 生命周期 submitted/working/completed/failed/cancelled | 继承 | 见 §3.3 状态机 |
| input_required/auth_required 状态 | 裁剪 | v1 不产生（权限瀑布在桥内闭环）；rejected=权限或门禁拒绝 |
| Message/Part（role + text/file/data） | 继承 | 三件 parts；v1 上行仅 TextPart，File/Data 仅接收 |
| 一条消息多 part | 裁剪 | v1 一条消息限 1 个 part |
| thoughts/tool_call | 负空间 | 不进 wire，桥内执行仅状态可见 |
| artifact 流转 / pushNotificationsConfig | 裁剪 | v1 不做；变更通知由 card 相 push 帧承担 |
| JSON-RPC 2.0 帧形与 tasks/* 方法名 | 继承 | 传输由 HTTP 改为 P2P 协议流（varint 帧） |
| 发现（HTTP registry） | 改义 | rendezvous namespace `a2a/agents/1` 签名注册 + mDNS 节点发现 + 已连 peer 推卡 |

发现与邀请的衔接：公开 agent 经 rendezvous 注册可发现；私有 agent 不进公共池，经签名凭证邀请点对点投递，受邀方同意后双方登记（宿主授权清单 + 订阅簿），随后走 card 相可见/ task 相可聊；撤销授权后宿主经 push 帧 removed 通知移除。节点通知链：PeerConnected 触发双向 subscribe↔list 即时同步，变更经 push 送达。

## 2. 线格式

底座帧封装（wire-format.md）单帧承载，载荷上限 1 MiB。开流首帧为协议 ID（`/a2a/1` UTF-8）。服务端以首帧嗅探分流：JSON 含 `op` 字段为 card 相帧，无 `op` 为 task 相 JSON-RPC 帧。裸流（无身份上下文）必须显式失败。

### 2.1 card 相帧表（serde tag "op"，snake_case；v=1）

| 方向 | op | 字段 | 语义 |
|---|---|---|---|
| C→S | list | v,id | 拉取对请求方可见卡片全集 |
| C→S | get | v,id,agent_id | 单卡查询；不可见即错误 |
| C→S | subscribe | v,id | 订阅变更；应答含 ok 帧与当前快照 push 帧 |
| C→S | invite_request | v,id,invite | owner 请求生成/登记邀请帧 |
| C→S | invite_receipt | v,id,receipt | invitee 提交签名回执 |
| S→C | cards | v,id,cards | list/get 应答（id 回显） |
| S→C | ok | v,id,subscribed | subscribe 应答 |
| S→C | push | v,id,cards,removed | 变更推送；id=0 即通知无需应答；removed 为卡片键数组（形 `hostPeer/agentId`） |
| S→C | error | v,id,code,message | 错误应答（id 回显） |
| S→C | invite_response | v,id,invite?,error?,message? | 邀请应答 |
| S→C | invite_receipt_response | v,id,ok,message? | 回执应答 |

客户端收到服务端帧型（cards/ok/push/invite_response/invite_receipt_response）属协议违规：服务端不回应并审计留痕。

### 2.2 卡片与签名信封

AgentCard 字段（camelCase，deny_unknown_fields，未知字段拒收）：agent_id（仅 [a-z0-9-]，≤32 字符）、name、description（均非空）、url（必须以 `a2a://<host_peer>/` 开头）、host_peer（base58）、visibility（public|private|local）、capabilities（v1 仅 streaming 布尔位）、skills（≤10 条；每条 id 仅 [a-z0-9-]、name 非空、description/tags 可选）、ttl_secs（0 < ttl ≤ 3600）、version（u64，订阅侧按版本升序替换）。capabilities/skills 是宿主自述，消费方必须按「未声明=不支持」处理。

SignedCard 信封：payload（卡片）、issued_at（unix 秒）、pubkey（b58 32B）、sig（b58 64B）。签名前像为 canonical payload + issued_at 小端 8 字节。验签必须含：pubkey 推导 PeerId == host_peer、Ed25519 验签、时间窗 now ∈ [issued_at, issued_at+ttl)。

### 2.3 task 相帧（JSON-RPC 2.0）

请求 `{jsonrpc:"2.0", id:u64, method, params}`；方法集：tasks/create、tasks/send、tasks/get、tasks/cancel。通知（无 id）：`tasks/status`（params: taskId,state）、`tasks/message`（params: taskId,messageId,message）。应答 `{jsonrpc:"2.0", id, result|error}`，error 体 `{code:i64, message:string}`。

Message：`{role: user|agent, parts:[Part]}`；Part 按 `type` 判别：text（{text}）、file（{name,mimeType,bytes}，bytes 为 base64）、data（{data:JSON}），均 deny_unknown_fields。

## 3. 时序与状态机

### 3.1 card 相时序

短连接请求-响应或长连接订阅态。subscribe 后同一流双向合流：请求继续可发，推送经订阅通道下发（通道容量 16，满即丢帧留 WARN，订阅端经下次拉取/心跳自愈）。重复 subscribe 幂等（回当前快照）。可见性过滤 fail-closed：list/push 只含请求方可见卡（public 全集；private 仅宿主授权清单内；local 仅 owner）；无可见卡对 list 回空集。心跳：宿主按卡片 TTL/2（默认 300/2=150 秒）向已订阅方推卡刷新；2×TTL 无心跳订阅侧除名（订阅簿寿命与发现 TTL 解耦）。公开池发现：宿主按卡 TTL 周期 rendezvous 签名注册 namespace `a2a/agents/1`；订阅侧退避拉取后逐个取卡验签入簿。

### 3.2 task 相时序

客户端开流后首帧必须为 tasks/create（或只读 tasks/get 恢复）。create 成功：服务端生成 taskId（UUID v4 simple 形态），spawn 专属子进程，回 result（taskId）+ status 通知（working）；首条消息随即入桥执行。执行期 agent 产出以 tasks/message 通知流出（messageId 供客户端去重，同 messageId 增量拼接）。终态经 tasks/status 通知回写，任务出并发簿（快照留簿服务 tasks/get）。EOF（客户端断流）时活跃任务必须 cancel 兜底（子进程停机）后收尾，孤儿进程不过夜。同一流二次 create 必须拒绝（one-task-per-stream）。

### 3.3 task 状态机与非法迁移

合法迁移：submitted→working；submitted→rejected；working→completed；working→failed；working→cancelled。终态（completed/failed/cancelled/rejected）不可再迁移；其余一切迁移必须拒绝并留错误。tasks/send 仅 working 态可追加用户消息；终态后 send/get 之外的写操作拒绝。

### 3.4 资源门禁

每 peer 并发 task 流 ≤4；每 task 累计上行输入 ≤262144 字节（256 KiB）；FilePart 单文件 ≤4194304 字节（4 MiB）；终态任务快照留簿上限 256 条，超出逐最旧终态。

## 4. 错误语义

card 相 error 帧码（String）：`not-found`（get 目标不可见或不存在）、`denied`（非 owner 发起邀请等权限拒绝）、`bad-card`（卡片非法）。邀请相错误经 invite_response.error / invite_receipt_response：`bad-invite`（帧解析或验签失败）、`nonce-used`（一次性 nonce 重放）、`store-error`（登记失败）；回执还应校验回执签名者必须是 invitee，不符即失败。线上错误不泄露内部细节，细节进审计日志。

task 相 JSON-RPC 错误：`-32700` 解析失败（id 缺失）；业务错误统一 `-32000`，机器可读码经 message 前缀承载：`one-task-per-stream`、`no-task-on-stream`（未 create 先 send/cancel）、`task-mismatch`（send 的 taskId 与本流不符）、`invalid-params`（缺 taskId/message/agentId）、`unknown method`、`gate-denied`（可见性门禁拒绝，含 agent 不存在与未启用）、每 peer 并发超限（per-peer/total）。收到请求必须应答（除 parse error 外 id 回显）；流中断按 §3.2 EOF 兜底处理。

## 5. 安全考量

可见性 fail-closed：远程仅 public + 授权清单内 private；local 仅 owner。private 任务相门禁双查：先 peer 级 authz.check(a2a.invoke)（读失败必须拒绝），再 agent 级授权清单；owner 免查。任务按创建者 PeerId 鉴权：send/cancel 限本流当前任务；tasks/get 同 peer 只读且可跨流（断线恢复）。权限路由：public/private 远程 agent 的敏感操作走宿主 owner 批准（超时即拒绝，默认拒绝），绝不经远程请求方批准；local（owner 本机）全权。卡片与邀请全链签名：卡片信封 peer 绑定防冒名；邀请帧 = SignedCard + 一次性 nonce（宿主持久化防重放）+ expiry（≤86400 秒）+ invitee 绑定；回执 = invitee 签名（nonce, agentId, hostPeer），双方登记后方可聊。AgentBook 入簿钳制：容量 256（超出逐最旧）、issued_at 偏差 >300 秒拒、ttl >3600 拒、版本不升序拒。

## 6. 兼容与版本

card 相帧带 v 字段（当前 1）；未知 op 必须显式失败。task 相兼容 JSON-RPC 2.0 语义：请求方必须忽略应答中的未知字段；方法集只增；新增状态值接收方必须按未知状态显式失败（不得当终态）。卡片/parts/message 为 deny_unknown_fields，未知字段拒收以防协议漂移；既有字段语义冻结，不兼容变更升版 /a2a/2 并存过渡。探测方式：开流写协议 ID，对端不支持即失败。

## 7. 测试向量

无（候选：a2a-card-frames.json card 相帧金样本与签名卡片；a2a-task-rpc.json JSON-RPC 序列与错误映射）。

## 8. 实现状态与出处

- 模型/卡片/信封：crates/a2a/src/card.rs（AgentCard/SignedCard/常量 AGENT_ID_MAX_CHARS=32、TTL_MAX_SECS=3600、TTL_DEFAULT_SECS=300、SKILLS_MAX=10）。
- card 相帧：crates/a2a/src/frame.rs（CardFrame 全型、task 相 JSON-RPC 类型）。
- 订阅簿：crates/a2a/src/book.rs（AgentBook、BOOK_MAX_ENTRIES=256、ISSUED_AT_MAX_SKEW_SECS=300）。
- task 状态机/parts：crates/a2a/src/task.rs（Task/TaskState/allowed_transition、TASKS_PER_PEER_MAX=4、TASK_INPUT_CAP_BYTES=262144、FILE_PART_CAP_BYTES=4194304）。
- 邀请帧：crates/a2a/src/invite.rs（InvitePayload/ReceiptPayload/INVITE_EXPIRY_MAX_SECS=86400）。
- 宿主接线：apps/acp-agent/src/a2a/handler.rs（首帧嗅探分流、订阅通道容量 16、心跳 TTL/2）、stream.rs 与 stream_dispatch.rs（task 流循环与错误映射、ERR_SERVER=-32000）、task.rs（授权双查、并发簿、REGISTRY_MAX=256）、invites.rs、publish.rs、bridge*.rs（task⇄子进程桥）。

漂移登记（代码为准，逐条）：

1. 设计文档与 wire-protocol §3.2 称 card 相「list/get/subscribe/push/remove 五动作」：实现中无独立 remove 帧——CardFrame 无 Remove 变体，移除语义并入 push 帧的 removed 数组（客户端动作实为 list/get/subscribe 三项，push/remove 为服务端帧型）。
2. 设计文档 §5.2 错误示例用 -32602（invalid params）：实现统一业务码 -32000 + message 前缀（invalid-params 等），-32602 未使用；-32700 仅用于解析失败。
3. tasks/get 快照 result 含 taskId 与 agentId（设计记为 {state,messages}），加法字段；上行输入累计计数（input_used）不进快照。
4. 设计 §7.3 私有邀请「投递复用 relay 打洞信令转发模式」：实现为 card 相流内四帧（invite_request/invite_response/invite_receipt/invite_receipt_response），邀请生成校验、nonce 一次性登记、回执验证与授权落表均在宿主 card 相 handler 内闭环；未走 relay 信令通道。
5. 客户端发送服务端帧型为协议违规：实现不回应、审计留痕（设计未明示该处置）。
6. 订阅推送通道容量 16 满即丢帧（WARN）为实现补充的资源防线，设计未定量。
7. EOF 兜底 cancel（断流任务必收敛、孤儿子进程不过夜）为实现补充语义；tasks/cancel 是显式取消入口，两条路径最终都落 cancelled。
8. task 相通知仅 tasks/status 与 tasks/message 两法（无 tasks/create 通知变体）；create 的应答与 working 通知在 create 请求流上同步回发。
