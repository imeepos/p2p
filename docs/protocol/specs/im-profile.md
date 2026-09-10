# /im/profile/1 规范

状态：stable；自 2026-09-08；归属 crates/p2p-chat；符合性：Core=GET/RESP 帧类型头与字段、一问一答时序、纯读语义、长度防线（name/description/avatar 上限）、id 匹配校验、10 秒超时；Extended=avatar data URL 字段（可缺省）。

## 1. 概览

本协议承载对端节点展示资料的按需查询：name（显示名）、description（简介）、
avatar（头像，data URL）。资料不随发现协议广播，仅在需要展示时一问一答拉取，
查询结果由调用方自行缓存或丢弃，本协议不定义订阅与失效通知。

一条流 = 一次问答（GET → RESP 后关流），半双工，无多轮。帧纪律与 /im/invite/1、
/im/ginvite/1 同构。资料为对端自报内容，无真实性保证（见 §5 采信边界）。

## 2. 线格式

帧封装复用 docs/protocol/wire-format.md §2：varint 长度前缀 + payload，单帧
payload ≤ 1 048 576 字节（含类型头）。payload 首字节为类型头，其余为 JSON：

| 类型头 | 值 | 载荷 |
|---|---|---|
| GET | 0x01 | 请求帧 JSON：`{id}` |
| RESP | 0x02 | 应答帧 JSON：`{id, ok, profile?, reason?}` |

### 2.1 帧字段表（camelCase）

| 字段 | 所属帧 | 类型 | 取值域与约束 |
|---|---|---|---|
| id | GET/RESP | string | UUID（v4），请求方生成；RESP.id 必须与请求 id 一致 |
| ok | RESP | bool | true = 查询成功（profile 必须存在）；false = 对端拒绝，reason 必须给出 |
| profile | RESP | object? | `{name, description, avatar?}`；ok=true 时必须存在，缺省即协议违规 |
| reason | RESP | string? | ok=false 时的可读拒绝原因；ok=true 时缺省 |

### 2.2 profile 字段与长度防线

| 字段 | 类型 | 上限（字符数） | 缺省含义 |
|---|---|---|---|
| name | string | ≤ 64 | 未设置为空串 |
| description | string | ≤ 280 | 未设置为空串 |
| avatar | string? | ≤ 200 000（data URL 字符） | null/缺省 = 未设置头像 |

长度以字符数计（非字节）。任一字段越界即协议违规，应答整体按 §4 拒绝处理，
不得截断后采信。

## 3. 时序与状态机

请求方：连接 → 开流 → GET → 读 RESP（单次问答全程 10 秒超时）→ 校验（id 匹配
→ ok → profile 存在 → 长度防线）→ 关流。查询持每 peer 串行锁，与聊天投递互斥。

应答方：读首帧（必须为 GET，否则断流）→ 读取本机资料 → 校验本机资料 → 回
RESP → 关流。应答恒为 ok=true：

- 本机已设置资料且未越界：回该资料。
- 本机未设置资料：回空资料（name/description 空串、avatar null），不是错误。
- 本机资料越界（长度防线同 §2.2）：必须降级回空资料并留告警，不得中断节点、
  不得回越界内容。

状态机无中间态：一问一答即结束，无分页、无续传、无订阅。

## 4. 错误语义

| 违例 | 检测方 | 行为 |
|---|---|---|
| 首帧非 GET / 帧缺类型头 / JSON 非法 | 应答方 | 断流 + 告警 |
| 本机资料越界 | 应答方 | 降级回空资料（ok=true）+ 告警 |
| RESP.id ≠ 请求 id | 请求方 | 协议违规，查询失败 |
| ok=false | 请求方 | 对端拒绝，reason 透出给调用方，查询失败 |
| ok=true 但缺 profile | 请求方 | 协议违规，查询失败 |
| profile 任一字段越界 | 请求方 | 按协议违规拒绝，不得截断采信 |
| 全程超 10 秒 | 请求方 | 超时失败 |

本协议纯读：任何一侧都不得因本协议产生落盘、本地事件或 outbox 挂起；查询
失败由调用方决定是否重试，协议层不重投。

## 5. 安全考量

自报性质：name/description/avatar 均为对端自行声明，本协议仅做长度防线，不做
真实性、一致性或内容合规验证。展示层（GUI）自行取舍是否采信、是否缓存、
是否过滤；缓存者必须自备失效策略（本协议无订阅/失效通知）。

资源上限：单帧 ≤ 1 MiB；name ≤ 64、description ≤ 280、avatar ≤ 200 000 字符，
avatar 为 data URL，200 000 字符约束同时限制了内嵌图片体积，防超大头像拖垮
展示与内存；10 秒问答超时防挂起连接堆积；每 peer 串行锁防并发查询风暴。

## 6. 兼容与版本

- JSON 载荷未知字段必须忽略；演进仅限加法可选字段（如 profile 新增可选展示
  字段，旧端忽略）。
- 纯读语义与"未设置 = ok 空资料"不得变更：它们是本协议无状态、可缓存、
  可被任意频次调用的前提。
- 不兼容变更升 /im/profile/2，新旧 ID 并存路由一个过渡期；旧页保留并标注。

## 7. 测试向量

无。registry 未为本协议登记向量集（两帧 JSON 结构已在 §2 表格给全，边界值
即 §2.2 长度防线）；后续如需防回归向量，须先在 vectors/README.md 登记 schema
并更新 registry.vectors 后按章程 §8 补齐。

## 8. 实现状态与出处

- 帧结构、入站 handler、越界降级：crates/p2p-chat/src/profile_wire.rs
- profile 模型与长度防线常量（64/280/200 000）：crates/p2p-chat/src/profile.rs
- 客户端问答、10 秒超时、每 peer 串行锁：crates/p2p-chat/src/profile_api.rs、
  core.rs

漂移登记（只登记，不在本轮改码）：

1. wire-protocol.md §8.5 称 RESP 的 profile 为可选（profile?）；实现中应答方
   ok=true 恒携带 profile，请求方将"ok=true 缺 profile"判为协议违规
   （profile_wire.rs:129-131），本页 §2.1/§4 已按代码收紧表述。
2. §8.5 未写明 avatar 上限以字符计且与 data URL 体积的关系，本页 §2.2/§5 已
   补全（代码以 chars().count() 校验，profile.rs:40）。
