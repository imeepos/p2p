# /im/chat/1 规范

状态：frozen；自 2026-09-03；归属 crates/p2p-chat；符合性：Core=帧类型头表、信封必选字段与取值域、事务时序、ACK 回执、幂等去重、sender 校验断流；Extended=媒体附件（MEDIA_BEGIN/MEDIA_CHUNK 路径）、replyTo 引用回复、card 入群邀请卡片、fromAddrs 地址自学习。

## 1. 概览

本协议承载好友间 1:1 私聊消息的投递：文本、附件、引用回复与入群邀请卡片。
一条出站流 = 一个消息事务（send-once）：发端开流、写信封帧、有附件再写媒体帧、
读对端 ACK 后关流；不复用流、不发端推送确认之外的任何回执。

与相邻协议的关系：/im/group/1 的消息事务与本协议帧纪律同构（群信封多 groupId、
sender 语义不同）；入群邀请卡片经本协议信封 kind=groupinvite 承载（载荷字段见
/im/ginvite/1 规范）；好友关系的建立走 /im/invite/1，本协议不校验好友簿状态。

本页状态 frozen（存量冻结）：不得修改既有字段与帧的语义，不兼容变更走
/im/chat/2 升版；兼容加法仅限新增可选字段（收端必须忽略未知字段）。

## 2. 线格式

帧封装复用 docs/protocol/wire-format.md §2：varint 长度前缀 + payload，单帧
payload ≤ 1 048 576 字节（1 MiB，含类型头）。payload 首字节为类型头：

| 类型头 | 值 | 载荷 |
|---|---|---|
| ENVELOPE | 0x01 | 信封 JSON，单帧，必须为事务首帧 |
| MEDIA_BEGIN | 0x02 | 媒体头 JSON：`{len, name, mime, kind}`，单帧 |
| MEDIA_CHUNK | 0x03 | 原始附件分片，无 JSON 封装；每帧数据 ≤ 1 048 575 字节 |
| ACK | 0x04 | 回执 JSON：`{id, ok, reason?}`；ok=false 时 reason 必须给出 |

### 2.1 信封 JSON 字段表（ENVELOPE 载荷，camelCase）

| 字段 | 类型 | 必选 | 取值域与约束 |
|---|---|---|---|
| id | string | 是 | UUID（v4），发端生成；收端幂等去重键 |
| peer | string | 是 | 发端自身 PeerId，base58；必须可解析且不得指向收端本机 |
| sender | string | 是 | 发端视角恒为 `"me"`；收端收到非 `"me"` 即伪装，断流 |
| kind | string | 是 | `text` / `image` / `audio` / `video` / `file`（小写）；`groupinvite` 为加法 kind，仅随卡片流程发出 |
| tsMs | i64 | 是 | 发端本地毫秒时间戳 |
| text | string? | 条件 | kind=text 时必须存在，trim 后 1..=2000 字符；kind≠text 时必须缺省或 null |
| media | object? | 条件 | `{name, mime, size}`；kind ∈ {image,audio,video,file} 时必须存在；kind=text/groupinvite 时携带即拒绝 |
| replyTo | string? | 否 | 被引用消息 id；null/缺省 = 无引用；收端原样落盘，不校验存在性 |
| card | object? | 否 | 加法字段（IMC1）：kind=groupinvite 时的卡片载荷；形状与语义见 /im/ginvite/1 规范 |
| fromAddrs | string[]? | 否 | 加法字段（F1 地址自学习）：发端可回拨地址；收端可回写好友簿，空则缺省 |

status/path 为本地存储字段，不得上 wire。media.size 为附件原始字节数。

### 2.2 附件与 MIME 白名单

单条消息（含附件原始字节）总量 ≤ 67 108 864 字节（64 MiB）。超限发送前必须拒绝，
入站必须断流。附件 size 必须 > 0。MIME 经 trim 与小写化后按 kind 精确匹配：

| kind | 允许的 MIME | 其他 MIME |
|---|---|---|
| image | image/png, image/jpeg, image/gif, image/webp | 拒绝 |
| audio | audio/mpeg, audio/wav, audio/ogg, audio/m4a, audio/mp4 | 拒绝 |
| video | video/mp4, video/webm, video/mov, video/quicktime | 拒绝 |
| file | 不限（仅校验 size） | - |
| text / groupinvite | 不得携带附件 | 任何附件即拒绝 |

## 3. 时序与状态机

发端（一条流）：连接 → 开流 → ENVELOPE →（有附件：MEDIA_BEGIN → MEDIA_CHUNK×n，
逐帧 ≤ 1 048 575 字节，总分片和必须等于 MEDIA_BEGIN.len）→ 读 ACK → 关流。
等待 ACK 超时 10 秒；ACK.id 必须等于信封 id，否则按协议违规处理；ok=false 即
本次投递失败，消息落 failed 态。

收端（一条流）：首帧必须为 ENVELOPE（否则断流）→ 校验信封（字段、sender、
peer、kind/text/media 约束、MIME 白名单）→ 幂等检查 → 收媒体分片并与 len 核对
→ 回 ACK(ok=true) → 落盘并投递本地事件（去重命中时仅回 ACK，不落盘不事件）
→ 关流。

任意帧校验失败（帧超上限、类型序非法、信封缺字段或校验不过、媒体长度不一致）
必须断流并留告警日志，不得静默吞错后继续按正常状态机推进。

## 4. 错误语义

本协议无独立错误帧；所有错误以"断流 + 告警日志"表达，或以 ACK(ok=false) 表达：

| 违例 | 检测方 | 行为 |
|---|---|---|
| 首帧非 ENVELOPE / 帧缺类型头 | 收端 | 断流 + 告警 |
| sender 非 `"me"` 或 peer 指向收端本机 | 收端 | 视为伪装，断流 + 告警 |
| 信封 JSON 非法 / 必选字段缺失 | 收端 | 断流 + 告警 |
| text 缺失、为空、trim 后超 2000 字符 | 收端 | 断流 + 告警 |
| kind 与 media/text 组合非法 | 收端 | 断流 + 告警 |
| MIME 不在 kind 白名单 / size=0 / size>64 MiB | 双方 | 发送前拒绝；入站断流 |
| 媒体分片总长 ≠ MEDIA_BEGIN.len / 单分片超限 | 收端 | 断流 + 告警 |
| 读到非 ACK 类型头（发端侧） | 发端 | 断流，投递失败 |
| ACK.id 与信封 id 不匹配 | 发端 | 协议违规，投递失败 |
| 等待 ACK 超 10 s | 发端 | 超时，投递失败 |
| groupId 未知等对端拒绝 | 收端 | ACK ok=false + reason 后关流 |

发端投递失败必须将消息置为可观测的失败态，不得静默丢弃。

## 5. 安全考量

身份边界：底座 handler 拿不到流对端 PeerId，发端身份由载荷 peer/sender 自声明，
流机密性与完整性由传输层（QUIC/TLS 或 TCP/Noise）保证。收端纵深防御必须执行：
sender 非 `"me"`、peer 指向本机均按伪装断流；对端身份最终以 peer 字段落盘归档。

资源上限：单帧 ≤ 1 MiB、单消息 ≤ 64 MiB、text ≤ 2000 字符、MIME 白名单按 kind
精确匹配（不猜测不降级），防附件类型混淆与无界内存。未知群等拒绝路径必须
零落盘。滥用面：重复 id 重放仅造成重复 ACK，不重复落盘（幂等去重挡板）。

## 6. 兼容与版本

JSON 载荷未知字段必须忽略（serde 默认语义），本协议演进全走加法：

- replyTo（首版冻结后加法）：被引用消息 id。旧端反序列化忽略未知字段照常收信；
  新端读旧信封（无该字段）得无引用。双向兼容，不影响 frozen 语义。
- card：kind=groupinvite 卡片载荷，旧端忽略后按无卡片文本处理。
- fromAddrs：带 serde default，旧对端缺字段可读；新端发旧端多余字段被忽略。

本页 frozen：既有字段、类型头值、时序不得改义；不兼容变更升 /im/chat/2，新页
新建、旧页保留并在顶部标注 Superseded by，两 ID 并存路由一个过渡期。

## 7. 测试向量

依据 docs/protocol/vectors/im-chat-envelope.json（信封 → ENVELOPE 帧字节、ACK 帧、
MIME 白名单拒绝样例；文件契约见 vectors/README.md）。该文件由规范化轮并行交付，
内容冻结前以 vectors/README.md 登记的 schema 为准。

## 8. 实现状态与出处

- 帧编解码与入站 handler：crates/p2p-chat/src/wire.rs
- 客户端事务与 ACK 超时（10 s）：crates/p2p-chat/src/core.rs
- kind/MIME/size 校验：crates/p2p-chat/src/kind.rs；text 校验与信封模型：
  crates/p2p-chat/src/model.rs；地址自学习：crates/p2p-chat/src/addr_learn.rs

漂移登记（只登记，不在本轮改码）：

1. wire-protocol.md §8.1 信封字段表未登记 card 与 fromAddrs 两个已上线加法字段
   （wire.rs:49-53），本页已按代码补全；§8.1 属内部文档，待对齐轮刷新。
2. §8.1 表述"附件 MIME 按 kind 精确白名单校验（…其余归 file）"易读作 file 也有
   白名单；实现中 file kind 不校验 MIME 仅校验 size（kind.rs:54），本页 §2.2 为准。
3. §8.1 称幂等"按消息 id 去重"；实现按本端 (peer, id) 存储键去重（wire.rs:269），
   语义等价（同 id 跨 peer 不冲突），本页 §3 表述为"按 id 去重"并注明存储键。
