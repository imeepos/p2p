# /im/ginvite/1 规范

状态：stable；自 2026-09-06；归属 crates/p2p-chat；符合性：Core=四帧类型头表、动作帧字段、owner-only 发起校验、邀请簿状态机、决策不可逆、离线挂起重投；Extended=note 备注、1:1 卡片消息联动（经 /im/chat/1 承载）。

## 1. 概览

本协议承载同意制入群邀请：群主（owner）邀请好友入群、受邀者同意或拒绝。
入群必须经受邀者明示同意，不存在静默拉人；实际入群动作由 owner 在收到
GACCEPT 后经 /im/group/1 的 roster 推送完成，本协议只做决策交换。

一条出站流 = 一次邀请动作（send-once）：连接 → 开流 → 写一个动作帧 → 读 ACK
→ 关流。帧纪律与 /im/invite/1 完全同构。

## 2. 线格式

帧封装复用 docs/protocol/wire-format.md §2：varint 长度前缀 + payload，单帧
payload ≤ 1 048 576 字节（含类型头）。payload 首字节为类型头，其余为 JSON：

| 类型头 | 值 | 载荷 |
|---|---|---|
| GINVITE | 0x01 | 邀请帧 JSON（owner 必须等于发端 peer，见 §3 纵深防御） |
| GACCEPT | 0x02 | 同意帧 JSON，同结构（reason 恒为 null） |
| GREJECT | 0x03 | 拒绝帧 JSON，同结构（reason 可携带拒绝理由） |
| ACK | 0x04 | 复用 /im/chat/1 AckFrame：`{id, ok, reason?}` |

### 2.1 动作帧 JSON 字段表（三类动作共用结构，camelCase）

| 字段 | 类型 | 必选 | 取值域与约束 |
|---|---|---|---|
| id | string | 是 | UUID（v4），发端逐帧生成；ACK.id 必须与之一致 |
| peer | string | 是 | 发端自身 PeerId，base58；不得指向收端本机 |
| groupId | string | 是 | 目标群 UUID |
| groupName | string | 是 | 邀请时的群名快照（供展示，非权威；权威以 roster 为准） |
| owner | string | 是 | 群主 PeerId；GINVITE 中必须等于 peer（owner-only 发起） |
| inviterNickname | string | 是 | 邀请者展示昵称快照 |
| note | string? | 否 | 邀请备注（Extended） |
| reason | string? | 条件 | GREJECT 语义携带拒绝理由；GINVITE/GACCEPT 恒为 null |

### 2.2 邀请簿（本地 group_invites.json，dataDir/chat/）

条目形状：`{id, groupId, groupName, owner, inviter, invitee, note?, direction:
"in"|"out", state: "pending"|"accepted"|"rejected", tsMs, delivered}`。
簿上限 256 条；重复邀请 upsert：同方向 + 同群 + 同对端视为同一条目——state
回 pending、delivered 复位、条目 id 保持稳定、tsMs 刷新。文件写纪律与
invites.json 一致（文件锁 + 原子替换）。簿满时必须拒绝新邀请并返回可读错误。

### 2.3 1:1 卡片消息联动（Extended）

owner 发起 GINVITE 的同时可经 /im/chat/1 发送卡片消息：信封 kind=groupinvite，
card 载荷 `{groupId, groupName, inviterNickname, note?}`，不得携带附件。卡片走
既有 1:1 离线投递（outbox），是通知渠道而非决策渠道：决策只能经本协议帧完成。

## 3. 时序与状态机

动作帧事务：连接 → 开流 → 写动作帧 → 读 ACK（超时 10 秒，ACK.id 必须匹配，
ok=false 即失败）→ 关流，全程持每 peer 串行锁。

状态机（邀请簿条目为状态载体）：

1. owner 发起：本地建 out 条目（pending，delivered=false），发 chat_group_invite
   事件；GINVITE 送达后标记 delivered。
2. 受邀者收 GINVITE：纵深校验（见 §4）→ upsert in 条目（pending，重复邀请刷新
   并重发事件）→ 发 chat_group_invite(pending) 事件；本机已在目标群时告警幂等
   忽略（回 ACK ok=true，不建条目）。
3. 受邀者同意：回 GACCEPT；本机条目暂保持 pending，待收到含自己在列的 roster
   （/im/group/1 推送）后置 accepted 并发事件。
4. 受邀者拒绝：本机立即置 rejected 并发事件，回 GREJECT（决策不可逆：rejected
   条目不得因任何后续帧回到 pending/accepted）。
5. owner 收 GACCEPT：校验本机为该群 owner → 复用 /im/group/1 邀请入群路径
   （roster rev+1 推全体含新成员）→ out 条目置 accepted 并发事件。无匹配 out
   条目、已拒绝条目、群已解散/非 active：告警幂等处理（回 ACK ok=true），不改
   决策状态。群满（32 人）时拒绝入群并回可读错误。
6. owner 收 GREJECT：out 条目置 rejected（幂等）并发事件，reason 留日志可观测。

离线语义：owner 离线时受邀者的 GACCEPT/GREJECT 挂起（delivered=false），经
outbox 重连重投 + 启动自愈 + 周期重投收敛；受邀者离线时 GINVITE 挂起同纪律，
并以 1:1 卡片消息（§2.3）先行通知。

## 4. 错误语义

| 违例 | 检测方 | 行为 |
|---|---|---|
| 帧缺类型头 / JSON 非法 / 未知类型头 | 收端 | 断流 + 告警 |
| GINVITE 的 owner ≠ 发端 peer | 收端 | 拒收（伪装嫌疑），处理失败回 ACK nack |
| GACCEPT 时本机非该群 owner | 收端 | 拒绝（伪装嫌疑），处理失败回 ACK nack |
| peer 指向收端本机 / 不可解析 | 收端 | 处理失败回 ACK nack |
| 重复 GINVITE | 收端 | 幂等：upsert 刷新 in 条目并重发事件，回 ACK ok=true |
| 已在群时收 GINVITE | 收端 | 告警幂等忽略，回 ACK ok=true |
| GACCEPT 无匹配 out 条目 / 条目已 rejected | owner | 告警幂等忽略，回 ACK ok=true（决策不可逆，不翻案） |
| GACCEPT 时群非 active / 已解散 | owner | 告警忽略，回 ACK ok=true |
| GREJECT 无匹配条目 | owner | 幂等回 ACK ok=true |
| 群满 32 人时执行入群 | owner | 拒绝入群，向受邀者回可读错误 |
| ACK.id 不匹配 / 等待 ACK 超 10 秒 | 发端 | 动作按失败处理，保持挂起重投 |

## 5. 安全考量

owner-only 是本协议核心信任假设：入群决策权只属于群主，发端必须在 GINVITE 中
自证（owner = 发端 peer），owner 侧对 GACCEPT 校验本机为该群 owner，双端一致
才可能推进 roster。伪造邀请/同意无法通过纵深校验，且 roster 收敛（/im/group/1
owner 绑定）使未授权成员名单无法落地。

资源上限：邀请簿 ≤ 256 条、单帧 ≤ 1 MiB；upsert 归一防同群重复邀请撑爆簿。
决策不可逆（rejected 终态）防止"先拒后潜"重放翻案；迟到 GACCEPT 必须忽略。
groupName/inviterNickname 为自报快照，仅展示用途，权威值以 roster 为准。

## 6. 兼容与版本

- JSON 载荷未知字段必须忽略；演进仅限加法可选字段。
- 与 /im/invite/1 帧纪律同构但协议 ID 独立，两者并存路由，互不影响升版。
- 不兼容变更升 /im/ginvite/2，新旧 ID 并存路由一个过渡期；旧页保留并标注。
- 卡片消息（§2.3）经 /im/chat/1 的 card 加法字段承载，其演进受 /im/chat/1
  frozen 约束：只能加字段，不得改义。

## 7. 测试向量

无。registry 未为本协议登记向量集（决策状态机为主、字节形状与 /im/invite/1
同构）；后续如需防回归向量，须先在 vectors/README.md 登记 schema 并更新
registry.vectors 后按章程 §8 补齐。

## 8. 实现状态与出处

- 帧结构与客户端投递：crates/p2p-chat/src/ginvite_wire.rs
- 入站状态机（GINVITE/GACCEPT/GREJECT 三 handler 与纵深校验）：
  crates/p2p-chat/src/ginvite_handler.rs
- 邀请簿条目、upsert 归一键、MAX_GROUP_INVITES=256、卡片结构：
  crates/p2p-chat/src/ginvite.rs、store_ginvite.rs
- 入群执行与 roster 联动（含满员拒绝）：crates/p2p-chat/src/ginvite_api.rs、
  ginvite_flow.rs
- 离线挂起与重投：crates/p2p-chat/src/outbox.rs

漂移登记（只登记，不在本轮改码）：

1. wire-protocol.md §8.4 称 GACCEPT"缺失匹配 out 条目仅告警回 ACK"，未区分
   "条目已 rejected"分支；实现将已拒绝条目的迟到 GACCEPT 单独忽略（决策不可逆，
   ginvite_handler.rs:164），本页 §3/§4 已按代码补全，§8.4 待对齐轮刷新。
2. §8.4 未登记群满 32 人时入群被拒的路径（ginvite_api.rs:222-224），本页 §4
   已补全。
