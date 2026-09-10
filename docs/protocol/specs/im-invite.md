# /im/invite/1 规范

状态：stable；自 2026-09-04；归属 crates/p2p-chat；符合性：Core=四帧类型头表、动作帧字段、一流一动作一 ACK 时序、互邀自愈、delivered 防重投、REJECT 幂等；Extended=无（本协议全部能力为好友建立所必需，无可选帧）。

## 1. 概览

本协议承载邀请制加好友的完整生命周期：发起邀请（INVITE）、同意（ACCEPT）、
拒绝（REJECT）与逐帧回执（ACK）。好友关系只能由双方同意建立，不存在单方面
加好友；本协议是 /im/chat/1、/im/group/1 等一切 IM 交互的前置信任建立环节。

一条出站流 = 一次邀请动作（send-once）：连接 → 开流 → 写一个动作帧 → 读 ACK
→ 关流。帧纪律与 /im/chat/1 的 ACK 路径同构。收端处理失败的回执语义见 §4。

## 2. 线格式

帧封装复用 docs/protocol/wire-format.md §2：varint 长度前缀 + payload，单帧
payload ≤ 1 048 576 字节（含类型头）。payload 首字节为类型头，其余为 JSON：

| 类型头 | 值 | 载荷 |
|---|---|---|
| INVITE | 0x01 | 邀请帧 JSON（addrs 必须携带发端当前可回拨地址） |
| ACCEPT | 0x02 | 同意帧 JSON，同结构（addrs 恒为空数组） |
| REJECT | 0x03 | 拒绝帧 JSON，同结构 |
| ACK | 0x04 | 复用 /im/chat/1 AckFrame：`{id, ok, reason?}` |

### 2.1 动作帧 JSON 字段表（INVITE/ACCEPT/REJECT 共用结构，camelCase）

| 字段 | 类型 | 必选 | 取值域与约束 |
|---|---|---|---|
| id | string | 是 | UUID（v4），发端逐帧生成；ACK.id 必须与之一致 |
| peer | string | 是 | 发端自身 PeerId，base58；必须可解析且不得指向收端本机 |
| nickname | string | 是 | 发端展示昵称；trim 后 ≤ 64 字符；空/越界时收端回退 PeerId 缩略，不得落原文 |
| addrs | string[] | 是 | 发端 multiaddr 列表。INVITE：当前 listen_addrs（供同意后回拨与自愈换址）；ACCEPT/REJECT：空数组 |

同一 JSON 结构承载三类动作，动作语义由类型头区分，载荷内不另设动作字段。

## 3. 时序与状态机

发起方（一条流）：连接 → 开流 → 写动作帧 → 读 ACK（超时 10 秒，ACK.id 必须
匹配，ok=false 即失败）→ 关流。INVITE 送达成功后本机标记 delivered。

收端（一条流）按类型头分发：

- INVITE 三分支（必须按序判定）：
  1. 已识好友（好友簿命中）：先登记帧内 addrs（节点簿 + 好友簿回写），再回投
     ACCEPT（自愈收敛，见下），回 ACK ok=true。
  2. 存在本机对对方的待同意邀请（互邀，out 方向 pending）：视为双方已同意——
     本机立即建好友、移除本机 out 条目、发 chat_invite(accepted) 事件、回投
     ACCEPT，回 ACK ok=true。
  3. 其余：登记来邀（in 条目，delivered=true），发 chat_invite(incoming) 事件，
     回 ACK ok=true，等待用户决策。
- ACCEPT 三分支：已识好友 → 幂等成功（清除残留 out 条目）；存在 out 待同意条目
  → 凭本机条目建好友（双向完成），移除条目，发 chat_invite(accepted)；两者皆无
  → 采信对端同意建好友（对端已同意，信任边界见 §5）并留告警，发
  chat_invite(accepted)。
- REJECT：移除本机 out 条目并发 chat_invite(rejected)；无匹配条目时幂等成功并
  留告警。

互邀自愈：对端重启换址后重发 INVITE（帧携带最新 addrs）→ 本机走"已识好友"
分支先登记新地址再回投 ACCEPT，保证回投不拨旧地址；双向建簿最终一致。

重投纪律：INVITE 由本机 outbox 联动重连重投与启动自愈；out 条目 delivered=true
后必须跳过重投（锁内复查，消除重投与用户同意/拒绝的竞态）；用户对同一对象
重复发起邀请时 upsert 刷新原条目并复位 delivered。回投 ACCEPT 为 fire-and-forget，
失败留告警，由对端重投 INVITE 再收敛。

## 4. 错误语义

| 违例 | 检测方 | 行为 |
|---|---|---|
| 帧缺类型头 / JSON 非法 / 首帧即未知类型 | 收端 | 断流 + 告警 |
| 未知类型头（非 0x01-0x04） | 收端 | 断流 + 告警 |
| peer 不可解析或指向收端本机 | 收端 | 处理失败，回 ACK ok=false + reason |
| 动作处理内部失败 | 收端 | 回 ACK ok=false + reason（不中断连接前先回执） |
| ACK.id 与动作帧 id 不匹配 | 发端 | 协议违规，动作按失败处理 |
| 等待 ACK 超 10 秒 | 发端 | 超时失败，等下次重连重投 |

INVITE 幂等：重复投递不产生重复 in 条目（upsert 覆盖刷新）。REJECT 幂等：无
匹配条目也必须回 ACK ok=true。ACCEPT 幂等：已识好友重复收到不重复建簿。

## 5. 安全考量

身份边界同 /im/chat/1：发端身份由载荷 peer 自声明，传输层保证机密性与完整性；
收端必须校验 peer 可解析且非本机（伪装防御）。ACCEPT 的"两者皆无"分支采信
对端同意直接建好友，是本协议唯一宽松路径：其前提是连接已经过传输层认证，
且仅建立好友簿条目，不产生任何权限提升；实现必须留告警保持可观测。

资源上限：单帧 ≤ 1 MiB；昵称 ≤ 64 字符；addrs 仅作回拨地址登记，收端必须做
地址卫生（禁止回写即弃监听地址污染好友簿——发端同样必须过滤后再上 wire）。
滥用面：重复 INVITE 重放只刷新 in 条目与事件，不放大存储；delivered 阻断重投
风暴。好友关系建立后仍可被 REJECT 之外的用户操作移除，本协议不提供删除语义。

## 6. 兼容与版本

- JSON 载荷未知字段必须忽略；演进仅限加法可选字段。
- 邀请簿（本地 invites.json）与好友簿同锁纪律；本协议线格式与簿结构解耦，
  簇内字段演进（如条目加 note）不影响线协议兼容性。
- 不兼容变更升 /im/invite/2，新旧 ID 并存路由一个过渡期；旧页保留并标注。

## 7. 测试向量

无。registry 未为本协议登记向量集（生命周期语义为主、字节形状简单，JSON
结构已在 §2 表格给全）；后续如需防回归向量，须先在 vectors/README.md 登记
schema 并更新 registry.vectors 后按章程 §8 补齐。

## 8. 实现状态与出处

- 帧结构、客户端投递与 delivered 锁内复查：crates/p2p-chat/src/wire_invite.rs
- 入站三分支语义（INVITE/ACCEPT/REJECT）与自愈回投：
  crates/p2p-chat/src/invite_handler.rs
- 邀请簿条目与 upsert：crates/p2p-chat/src/invite.rs、store_invite.rs
- 重连重投与启动自愈：crates/p2p-chat/src/outbox.rs

漂移登记（只登记，不在本轮改码）：未发现本协议 §8.3 与代码的行为性偏差；
§8.3 对"互邀即同意"的判定顺序（先查好友簿、再查互邀、最后登记 in）未展开，
本页 §3 已按代码补全判定次序。
