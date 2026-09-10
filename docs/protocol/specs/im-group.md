# /im/group/1 规范

状态：stable；自 2026-09-04；归属 crates/p2p-chat；符合性：Core=八类型头表、消息与 roster 事务、roster 收敛规则（owner 绑定/rev 单调/high rev 胜）、unknown_group 零落盘、(groupId,id) 幂等、members≤32 与群名上限；Extended=媒体附件路径（与 /im/chat/1 同构）、replyTo 引用回复。

## 1. 概览

本协议承载好友群聊：群消息投递、roster（成员名单）全量收敛、踢出与退出席。
群为 owner 权威模型：owner 是唯一 roster 权威，成员变更（邀请/移除/退群/解散/
改名）一律由 owner 递增 rev 并向全体成员推送全量 roster 收敛，无增量同步。

一条流 = 一个事务；首帧类型决定事务类型。与 /im/chat/1 并存路由（不同协议 ID），
消息事务帧纪律与 /im/chat/1 同构；好友关系由 /im/invite/1 建立，本协议不建好友。

## 2. 线格式

帧封装复用 docs/protocol/wire-format.md §2：varint 长度前缀 + payload，单帧
payload ≤ 1 048 576 字节（含类型头）。payload 首字节为类型头：

| 类型头 | 值 | 载荷 | 事务 |
|---|---|---|---|
| G_ENVELOPE | 0x01 | 群消息信封 JSON，单帧 | 消息 |
| MEDIA_BEGIN | 0x02 | 媒体头 JSON：`{len, name, mime, kind}`，单帧 | 消息（可选） |
| MEDIA_CHUNK | 0x03 | 原始附件分片；每帧 ≤ 1 048 575 字节 | 消息（可选×n） |
| ACK | 0x04 | `{id, ok, reason?}` | 消息回执 |
| G_STATE | 0x11 | roster JSON，单帧 | roster |
| G_STATE_ACK | 0x12 | `{groupId, rev, ok, reason?}` | roster 回执 |
| G_KICK | 0x13 | `{groupId, rev, reason}`，reason ∈ {kicked, disbanded} | 单向通知 |
| G_LEAVE | 0x14 | `{groupId, sender}` | 单向通知 |

### 2.1 群消息信封 JSON 字段表（G_ENVELOPE 载荷，camelCase）

| 字段 | 类型 | 必选 | 取值域与约束 |
|---|---|---|---|
| id | string | 是 | UUID，发端生成；收端幂等键（与 groupId 联合） |
| groupId | string | 是 | UUID，owner 建群时生成，全周期不变 |
| sender | string | 是 | 作者 PeerId，base58；必须可解析、不得指向收端本机，且必须 ∈ 收端本地 roster.members |
| kind | string | 是 | `text` / `image` / `audio` / `video` / `file`（小写，同 /im/chat/1） |
| tsMs | i64 | 是 | 发端本地毫秒时间戳 |
| text | string? | 条件 | kind=text 时必须存在，trim 后 1..=2000 字符；kind≠text 必须缺省 |
| media | object? | 条件 | `{name, mime, size}`；MIME 白名单同 /im/chat/1 §2.2 |
| replyTo | string? | 否 | 被引消息 id，不校验存在性 |

status/path/acks 为本地字段，不得上 wire。单条消息（含附件）≤ 67 108 864 字节
（64 MiB）。

### 2.2 roster JSON 字段表（G_STATE 载荷，camelCase）

| 字段 | 类型 | 取值域与约束 |
|---|---|---|
| groupId | string | UUID，与本端群条目主键匹配 |
| name | string | trim 后 1..=64 字符 |
| owner | string | 群主 PeerId，base58；必须可解析且不得指向收端本机 |
| members | string[] | 全量名单，含 owner；必须去重、≤ 32 条、必须含收端本机 |
| rev | u64 | 从 0 单调递增，仅 owner 有权递增 |
| tsMs | i64 | owner 生成时间戳 |

## 3. 时序与状态机

消息事务：G_ENVELOPE →（可选 MEDIA_BEGIN → MEDIA_CHUNK×n，总长必须等于
MEDIA_BEGIN.len）→ ACK；发端等待 ACK 超时 10 秒，ACK.id 必须匹配。
roster 事务：G_STATE → G_STATE_ACK；收端校验/收敛后回执（含 rev 原样回带），
校验失败回 ok=false + reason 后断流。
单向通知：G_KICK/G_LEAVE 无回执，best-effort；离线成员由发端 goutbox 补投，
roster 是权威收敛机制（补投丢失不破坏一致性，下次 roster 全量覆盖）。

收端 roster 收敛规则（必须全部执行）：

1. 首见 groupId：校验通过即建群（state=active），落定 owner。
2. 已有群且本机是 owner：拒收外来 roster（本机管理的群不认外来名单）。
3. 已有群且 roster.owner ≠ 本地记录 owner：拒收告警（owner 绑定，防换主伪装）。
4. roster.rev ≤ 本地 rev：幂等丢弃，仍回 G_STATE_ACK ok=true。
5. roster.rev > 本地 rev：高 rev 胜，整体覆盖 name/members/rev，state 回 active
   （支持被移出后重邀回归），并发本地事件。

## 4. 错误语义

| 违例 | 检测方 | 行为 |
|---|---|---|
| 首帧类型头未知（非上表八值） | 收端 | 断流 + 告警 |
| groupId 不在本端 groups.json | 收端 | 回 ACK ok=false reason=`unknown_group` 后正常关流，零落盘 |
| sender 不在本地 roster.members / 指向本机 | 收端 | 视为伪装，断流 + 告警 |
| roster 校验失败（群名/owner 指向本机/members 重复或超 32 或不含本机） | 收端 | G_STATE_ACK ok=false + reason，随后断流 |
| G_KICK reason 非 kicked/disbanded | 收端 | 断流 + 告警 |
| G_KICK/G_LEAVE 指向未知群 | 收端 | 告警忽略（幂等，无回执） |
| 本机是 owner 收到外来 roster/G_KICK | 收端 | roster 回 nack 断流；G_KICK 告警忽略 |
| 非 owner 收到 G_LEAVE / 退群者不在群 | owner | 告警忽略（幂等） |
| 媒体长度不一致 / 帧超限 / JSON 非法 | 收端 | 断流 + 告警 |
| ACK / G_STATE_ACK id+rev 不匹配 | 发端 | 协议违规，事务失败 |

发端事务失败必须保留消息为可重试的 pending 态，不得静默丢弃。

## 5. 安全考量

身份边界同 /im/chat/1：发端身份由载荷自声明（底座 handler 拿不到流对端
PeerId），传输层保证机密性与完整性；收端纵深防御必须执行 sender ∈ members、
owner 绑定、roster owner 非本机三项校验。owner 绑定 + rev 单调使伪造 roster
无法换主或回滚名单；高 rev 覆盖要求攻击者持有 owner 身份才能推进 rev。

资源上限：单帧 ≤ 1 MiB、单消息 ≤ 64 MiB、群名 ≤ 64 字符、members ≤ 32；
unknown_group 与未知群通知路径必须零落盘。重放（同 groupId+id）仅触发重复
ACK，不重复落盘不重复投递事件。

## 6. 兼容与版本

- G_LEAVE 的 sender 为加法字段（历史版本无此字段，底座身份缺口同源）；收端
  对缺 sender 的旧帧必须按协议违规拒绝或忽略，不得猜测退群者身份。
- JSON 载荷未知字段必须忽略；加法仅限新增可选字段，不改既有字段语义。
- 本协议不兼容变更升 /im/group/2，与 /im/group/1 并存路由一个过渡期。
- roster 补投最终一致：本协议不保证通知可达，一致性由 owner 周期/事件驱动的
  roster 重推收敛，实现方可自选重推节拍（不在线格式约束内）。

## 7. 测试向量

无。registry 未为本协议登记向量集（群聊字节级行为由 roster 收敛语义主导，
JSON 形状已在 §2 表格给全）；后续如需防回归向量，须先在 vectors/README.md
登记 schema 并更新 registry.vectors 后按章程 §8 补齐。

## 8. 实现状态与出处

- 帧编解码与入站分发 handler：crates/p2p-chat/src/group_wire.rs
- roster/kick/leave 帧结构与入站纵深校验：crates/p2p-chat/src/group.rs
- roster 收敛状态机（apply_roster/apply_kick/apply_leave）：
  crates/p2p-chat/src/group_model.rs
- 群存储、群名校验、MAX_GROUP_MEMBERS=32：crates/p2p-chat/src/group_store.rs
- 客户端事务与 goutbox 补投：crates/p2p-chat/src/group_core.rs、group_send.rs

漂移登记（只登记，不在本轮改码）：

1. wire-protocol.md §8.2 称"owner 本机管理的群拒收外来 roster/G_KICK"；实现中
   外来 roster 回 nack 断流（group_wire.rs:257），而外来 G_KICK 为告警忽略后
   正常返回（group_model.rs:132-135，无回执可 nack）。本页 §4 按代码分列，§8.2
   待对齐轮刷新。
2. §8.2 未写明 rev ≤ 本地时 G_STATE_ACK 仍回 ok=true 的确切回执形状已写明
   （ok=true），代码与 §8.2 一致；本页补充了 G_STATE_ACK 必须原样回带 groupId+rev
   的匹配校验（group_core.rs:193），属规格细化非漂移。
