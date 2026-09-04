# IM 群聊系统设计 v1（基于 1:1 好友聊天加法，2026-09-05）

状态：设计稿（待协调会话冻结后派单）。基线 [im-chat-design.md](im-chat-design.md)（v1 冻结）不动，
群聊全部为**加法**：新协议 `/im/group/1`（wire-protocol.md 实现时登记 §8.2）、新存储子树、
新命令面（gui-contract.md 实现时追加 §14）。1:1 的线协议、存储文件、命令签名零改动。

## 1. 定位与边界

群 = 好友边沿上的多播：无服务器、无全局共识，消息按成员逐个经既有 P2P 直连/降级链
fan-out。成员名单采用 **owner 权威模型**：群主是名单唯一权威（rev 单调递增），
成员被动接收 roster 并落盘；所有权不转移、不多管理员。

- **做（v1）**：建群（好友簿勾选）、邀请/移除成员、成员退群、owner 解散、群改名、
  文本+emoji 消息、四类附件、per-member 离线队列（复用 outbox 纪律）、群历史分页、
  per-member 送达明细（acks）。
- **不做（v1）**：@提及、已读回执、消息撤回/编辑、多管理员/所有权转移、邀请确认流、
  新成员历史回填、群头像、跨端同步、大群。
- **硬边界**：群上限 `32` 成员（fan-out 成本线性，最坏 31×64MiB 带宽是上限的直接理由）；
  单条消息（含附件）≤ `64 MiB` 与 1:1 一致；群名 trim 后 1..=64 字符；成员必须是
  好友簿在册节点（可拨性前提）；groupId = UUID（owner 生成）；目标永不等于本机 PeerId。

## 2. 复用面（底座与 1:1 既有件只读）

| 既有件 | 群聊用法 |
|---|---|
| `Node::connect` / `new_stream` / `handle_protocol` | 群流量走新协议 ID，传输原语同一套 |
| chunked 帧封装（varint 长度前缀 + 类型头） | `/im/group/1` 同风格帧封装 |
| ChatCore per-peer 串行锁、ACK 10s 超时、强拆重拨 | 群消息 per-member 投递同一纪律 |
| outbox 任务（PeerConnected flush、批量上限、死信） | 扩为双队列：1:1 队列不动，群消息走 goutbox |
| friends.json | 群成员来源与可拨性；非好友不能入群 |
| store_io（JSONL 追加/原子写/损坏行跳过） | 群存储同规范 |

## 3. 线协议 /im/group/1

新协议 ID 与冻结的 `/im/chat/1` 并存路由（wire-protocol.md §8.3 演进策略：新增协议
不改既有 ID 语义）。一条流 = 一个事务（send-once），帧 payload 首字节 = 类型头：

| 类型头 | 值 | 载荷 |
|---|---|---|
| G_ENVELOPE | 0x01 | 群消息信封 JSON（单帧） |
| MEDIA_BEGIN | 0x02 | 媒体头 JSON：{len, name, mime, kind}（单帧） |
| MEDIA_CHUNK | 0x03 | 原始附件分片（≤ 1 MiB - 1 字节，chunked 规则） |
| ACK | 0x04 | JSON：{id, ok, reason?} |
| G_STATE | 0x11 | roster JSON（单帧） |
| G_STATE_ACK | 0x12 | JSON：{groupId, rev, ok, reason?} |
| G_KICK | 0x13 | JSON：{groupId, rev, reason: "kicked"\|"disbanded"} |
| G_LEAVE | 0x14 | JSON：{groupId} |

- 消息事务：G_ENVELOPE →（可选 MEDIA_BEGIN → MEDIA_CHUNK×n）→ ACK —— 与 /im/chat/1 完全同构。
- roster 事务：G_STATE → G_STATE_ACK。G_KICK/G_LEAVE 为单向通知（best-effort +
  goutbox 重试），roster 才是权威收敛机制。
- 任意帧校验失败 → 断流 + 告警日志，发端该成员条目落 failed（同 1:1，禁静默）。

### 3.1 群消息信封（线上 JSON）

```json
{
  "id": "UUID（发端生成，收端拒绝接受对端 dict 的 id）",
  "groupId": "UUID 字符串（owner 生成）",
  "sender": "发端自身 PeerId（base58）",
  "kind": "text | image | audio | video | file",
  "tsMs": 0,
  "text": "kind=text 时有，trim 后 1..=2000 字符",
  "media": { "name": "...", "mime": "...", "size": 0 },
  "replyTo": "可选，被引消息的本端消息 id（同 1:1 语义，不校验存在性）"
}
```

status/path/acks 为本地字段不跨网（同 1:1 纪律）。sender 语义对齐 1:1 wire 的 peer
字段：底座 handler 拿不到流对端身份（im-chat-design.md §3 既有裁决），载荷声明发端，
收端做纵深校验（§3.3），缺口登记见 §9。

### 3.2 roster（线上 JSON）

```json
{ "groupId": "...", "name": "...", "owner": "PeerId", "members": ["PeerId"], "rev": 0, "tsMs": 0 }
```

- rev 从 0 单调递增且仅 owner 递增；收端**高 rev 胜**，≤ 本地 rev 的 roster 幂等丢弃。
- members 全量名单（含 owner），去重，≤32，不含本机 → 拒收告警。
- owner 绑定：本地无此群时，首个 roster 落定 owner；已有群时 roster.owner ≠ 本地
  owner → 拒收 + 告警（防换主伪装）。

### 3.3 入站校验（收端纵深防御）

- 信封：sender base58 合法且 ≠ 本机；kind/文本/媒体同 1:1 白名单与上限。
- 群消息：groupId 必须在本地 groups.json，否则回 ACK ok=false reason=unknown_group
  （发端该成员条目保持 pending 等 roster 后重试；owner 邀请流先推 roster 再发消息，
  正常时序不命中）；sender ∉ 本地 roster.members → 断流告警。
- 幂等：消息按 (groupId, id) 去重，重复仅回 ACK 不重复落盘；roster 按 rev 去重。

## 4. 本地存储布局（dataDir/chat/ 增量，1:1 文件零迁移）

```
chat/
├── friends.json              # 不动
├── groups.json               # 新增：我在的群，原子写 + 文件锁（同 friends 纪律）
├── outbox/<peer>.jsonl       # 不动（1:1）
├── goutbox/<peer>.jsonl      # 新增：群 per-member 离线队列，行 = {msg: 群信封, to: memberPeerId}
├── messages/<peer>.jsonl     # 不动（1:1）
├── groups/<groupId>.jsonl    # 新增：群消息历史（追加式 JSONL，损坏行跳过 warn）
└── media/<groupId>/...       # media/ 复用：1:1 子目录键 = peerId，群 = groupId（base58 ≠ UUID，命名空间不相交）
```

- groups.json 条目：{groupId, name, owner, members[], rev, state, tsMs}；
  state: "active" | "left" | "kicked" | "disbanded"。退群/被踢/解散**不删数据**——
  历史保留、state 置位（对齐 1:1 friend_remove 不删历史的先例），列表按 state 过滤展示。
- 群历史条目（本地形状，JSONL 一行一条）：群信封字段 + path?（附件本端落盘路径）+
  status + acks: string[]（已 ACK 成员 PeerId，仅本端发出的消息维护，收到的恒空）。
- 状态机（四态枚举复用 1:1）：pending（尚有成员条目未终态）→ delivered（acks ⊇ 目标
  全体）；全部目标成员终态失败 → failed；sent 不用于群消息（枚举保留不占用）。
  GUI 送达展示「已送达 |acks|/n」由 acks 推导，不新增状态值。

## 5. 成员操作语义（全部经 roster rev 收敛）

| 操作 | 发起人 | 动作 |
|---|---|---|
| 建群 | 任意节点 | 校验成员 ⊆ 好友簿且 ≤32 且不含本机 → 本地 rev=0 建群 → 对每个初始成员推 roster |
| 邀请 | owner | 校验受邀者 ∈ 好友簿、群 <32 → rev+1 → 推全体成员（含新成员） |
| 移除 | owner | rev+1 → 推余下成员；对被移者发 G_KICK(reason=kicked) |
| 退群 | 成员 | 本端立即 state=left（历史保留）；向 owner 发 G_LEAVE，owner 收后 rev+1 推余员（owner 离线则 goutbox 排队，最终一致） |
| 解散 | owner | rev+1 → 对全体成员发 G_KICK(reason=disbanded)；owner 本地 state=disbanded |
| 改名 | owner | 校验群名 → rev+1 推 roster |

- 非 owner 发起 owner 操作 → 显式 Err（可读中文）；owner 不能退群（退群即解散）。
- 成员端收 G_KICK：state 置 kicked/disbanded，群不可再发；历史保留。
- G_KICK/G_LEAVE 载荷也走 goutbox（离线 owner/成员重连后补投）。

## 6. 发送与离线投递语义

1. group_send：校验（群 active、本机在册、文本/媒体合法 ≤64MiB）→ 生成群信封 →
   群历史落 pending → 为每个其他成员写 goutbox 条目 → 对在线成员逐个 deliver
   （持 per-peer 串行锁，串行 fan-out）→ 每收到 ACK 计入 acks 并发 chat_group_status。
2. 连接失败：该成员条目保持 pending；PeerConnected 触发 goutbox flush（复用 outbox
   纪律：批量上限 32、failed 每进程一次重投机会、二次死信出队 + 告警）。
3. 显式拒绝（ACK ok=false）：该成员条目 failed，reason 留日志。
4. 入站：收完整（媒体落 media/<groupId>/）→ 回 ACK → 群历史追加 → chat_group_message 事件。
5. 历史：group_history(groupId, beforeId?, limit≤100) 同 1:1 游标语义（time desc）。
6. 重发安全：同一 message id 对每成员重发，收端 (groupId, id) 去重仅回 ACK（§3.3）。

## 7. GUI 契约加法（实现时同步 gui-contract.md §14，mock 同签名）

命令（camelCase，参数无效一律 Err 可读中文）：

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| group_create | name: string, memberIds: string[] | GroupJson | 校验（成员 ⊆ 好友簿、≤32、不含本机、群名 trim 1..=64）后本地建群并推 roster |
| group_list | - | GroupJson[] | 全量含 left/kicked/disbanded（GUI 按 state 过滤/置底） |
| group_invite | groupId: string, memberIds: string[] | GroupJson | owner-only；rev+1 推全体（含新成员） |
| group_kick | groupId: string, memberId: string | GroupJson | owner-only；rev+1 推余员 + G_KICK |
| group_leave | groupId: string | GroupJson | 本端 state=left；G_LEAVE 通知 owner |
| group_rename | groupId: string, name: string | GroupJson | owner-only；rev+1 推 roster |
| group_send | groupId, kind: ChatKind, text?, media?: ChatMediaInput, replyTo? | GroupSendReport | 校验→fan-out；见 §6 |
| group_history | groupId: string, beforeId?: string, limit?: number | GroupMessageJson[] | 同 1:1 分页语义 |
| group_media_file | groupId: string, messageId: string | { path, mime, name } | 同 1:1，目录为 media/<groupId>/ |

```ts
interface GroupJson {
  groupId: string;        // UUID
  name: string;           // trim 后 1..=64 字符
  owner: string;          // PeerId
  members: string[];      // PeerId[]，含 owner，≤32
  rev: number;
  state: "active" | "left" | "kicked" | "disbanded";
  tsMs: number;
}

interface GroupMessageJson {
  id: string;             // UUID（发端生成）
  groupId: string;
  senderId: string;       // 作者 PeerId；本端消息判定 senderId === 本机 PeerId
                          //（GUI 经既有节点信息命令取本机 PeerId，渲染路径复用 1:1 气泡）
  kind: ChatKind;
  tsMs: number;
  text?: string | null;
  media?: ChatMediaJson | null;   // 复用 §12.3 ChatMediaJson
  status: "pending" | "sent" | "delivered" | "failed";  // sent 不出现（§4 状态机）
  acks: string[];         // 已确认成员 PeerId（仅本端发出的消息非空）
  replyTo?: string | null;
}

interface GroupSendReport {
  message: GroupMessageJson;
  acked: number;          // 本轮已确认成员数
  recipients: number;     // 目标成员数（n-1）
  delivered: boolean;     // acked === recipients
}
```

事件（追加到判别联合）：

```ts
| { type: "chat_group_message"; groupId: string; message: GroupMessageJson }
| { type: "chat_group_status"; groupId: string; messageId: string; acks: string[]; status: "pending"|"delivered"|"failed" }
| { type: "chat_group_state"; group: GroupJson }   // roster 变更/踢出/解散/退群回执
```

## 8. 里程碑拆单（并行互斥，任务书只含需求与机械验收；G 前缀避开已占用 T 系列）

| 单 | 范围（目录） | 依赖 | 验收要点 |
|---|---|---|---|
| G1 | crates/p2p-chat 新增 group.rs / group_store.rs / group_wire.rs / group_core.rs + outbox 双队列 + Chat 装配 + tests/group_*.rs（建群/roster rev 收敛/双节点消息 roundtrip/附件/离线 flush/去重/kick/leave/owner 绑定拒收）+ wire-protocol.md §8.2 登记 | 无 | cargo test -p p2p-chat 全绿；clippy -D warnings；make check |
| G2 | apps/gui/src：ipc-types/ipc/mock 群段、i18n 中英、群聊空页壳 | 无（对 §7 契约编程） | pnpm build + pnpm test 全绿；i18n-diff 零漂移；make check |
| G3 | apps/gui/src/views/chat + components/chat + stores：会话列表 1:1/群混排、群管理面板（成员/邀请/移除/退群/解散）、群消息渲染（senderId→好友簿昵称解析） | G2 合入 | pnpm build + pnpm test 全绿；交互组件测试（建群/发文本/成员面板）；make check |
| G4 | apps/gui/src-tauri：group_* 命令 + 3 事件接线 + 契约 roundtrip 测试 | G1 | src-tauri cargo test 全绿；pnpm build + make check |
| G5 | crates/p2p-itest/tests/chat_group_e2e.rs（三节点：建群→roster→文本→附件→离线成员上线 flush→kick→leave→解散）+ docs/ops/im-group-drill.md | G1+G4 | cargo test -p p2p-itest --test chat_group_e2e 全绿；make check；演练清单在位 |

**行数预算（红线 300，现值 lib.rs 297 / core.rs 297 / model.rs 297 均无余量）**：
新文件各自 ≤280 行；outbox.rs 62 → 约 120；**lib.rs 禁止净增**——G1 首步把 lib.rs 内
friend_update 的长 doc 注释下沉 friend.rs（代码零变更）腾出 ≥4 行，再以 ≤3 行挂载
群模块（mod 声明 + pub use + 装配一行）；store.rs 不动（群存储全落 group_store.rs）。

## 9. 安全与已知缺口

- 传输层 TLS1.3/Noise 保证流安全；handler 拿不到流对端 PeerId（im-chat-design.md §3
  既有裁决缺口）。群聊收紧措施：roster owner 绑定（§3.2）、群消息 sender ∈ members
  校验（§3.3）。**残余缺口**：恶意节点仍可伪造 owner 身份推 roster、伪造成员发消息
  （与 1:1 伪装缺口同源，群场景放大）。底座为 handler 注入流对端 PeerId 后一并收紧：
  roster 流对端 == owner、消息流对端 == sender（冻结契约加法路径，登记待底座排期）。
- 带宽放大：fan-out 附件上传 ×(n-1)；32 人 × 64MiB 上限约束最坏情形；成员间协同分发
  （erasure/gossip）登记非 v1。
- 隐私：members 全量名单全员可见（P2P 群固有语义）；消息无服务器留存，仅成员本机。

## 10. 非目标登记（防范围蔓延）

@提及、已读回执、消息撤回/编辑、多管理员/所有权转移、邀请确认流、新成员历史回填、
群头像、跨端同步、大群（>32）、媒体协同分发。
