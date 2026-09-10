# /p2p-base/identify/1 规范

状态：draft（impl=planned，未实现）；自 2026-09-01；归属 crates/p2p-relay；符合性：暂无 Core/Extended 项（语义预留页）

依据 registry.toml：本协议当前仅有协议 ID 常量登记，无 handler、无消息结构、无线上实例。
**在本文升为 stable 之前，任何第三方实现不得依赖本协议；节点实现也不应当实现它。**

## 1. 概览

- 解决什么问题：传输安全握手只互认 PeerId，不交换地址信息；mDNS 仅局域网有效，
  rendezvous 需要注册服务端。identify 预留为点对点的「公钥 + 监听地址 + 观测地址」
  交换通道，服务于静态拨号对端、无发现基础设施等补充场景。
- 适用边界：补充性协议，不替代 mDNS/rendezvous；不承载身份认定（见下）。
- 与握手身份认定的关系：身份认定只来自握手（PeerId = base58(SHA-256(ed25519 公钥))，
  见 wire-format.md §4）。identify 未来定义的一切自报字段（含公钥、地址）都不得作为
  身份证据，只能作为待验证线索；两者的分工是「握手管身份、identify 管地址线索」。

## 2. 线格式

当前冻结事实仅两项：

1. 协议 ID 字符串 `/p2p-base/identify/1`，作为流首帧 payload 的 UTF-8 字节共 20 字节；
   按帧封装（wire-format.md §6）完整首帧为 21 字节：

       14 2f 70 32 70 2d 62 61 73 65 2f 69 64 65 6e 74 69 66 79 2f 31

2. 帧封装沿用 wire-format.md §6：无符号 varint 长度前缀 + payload，单帧上限
   1 048 576 字节；varint 前缀最长 10 字节。

未来载荷字段表：**尚未定稿，留空**。定稿时应当遵循控制面惯例（wire-protocol.md §7）：
载荷采用 protobuf，字段只增不改（tag 与类型冻结），并须在本文补齐字段表（tag、类型、
取值域与上限的确切值）后才能转 stable。

## 3. 时序与状态机

当前（未实现）的唯一可观测行为：

1. 任何一端在该协议上开流并写协议 ID 首帧；
2. 接收方在 handler 注册表查不到该 ID，立即关流并上报 UnsupportedProtocol
   （wire-format.md §8）；
3. 发起方观察到流被立即关闭，即「对端不支持本协议」。

因此当前不存在合法业务时序；未来定稿应当给出：一问一答的请求-应答时序、单流单事务
边界、每事务超时的确切值。当前无任何合法状态序列。

## 4. 错误语义

当前唯一错误语义：

| 信号 | 触发 | 收发双方确切行为 |
|---|---|---|
| UnsupportedProtocol（关流） | 接收方无本协议 handler | 接收方立即关流并上报；发起方必须视作「对端不支持」，不重试、不做猜测降级 |

未来实现的错误必须以显式信号定义在本文错误表（错误帧或错误码 + 双方确切行为），
不得静默吞掉后继续推进状态机。

## 5. 安全考量

- 自报地址不可信：接收方不得把对端自报的监听/观测地址当作已验证事实；地址入簿时
  应当降权，且按地址拨号必须携带期望 PeerId 校验（PeerMismatch 即断链，wire-format.md §4.4）。
- 观测地址应当由反射口等第三方观测学习（node-lifecycle.md §1.4），自报值仅作候选。
- 公钥字段（若未来定义）必须与握手推导 PeerId 逐字节比对一致才可采信，防止身份嫁接。
- 资源防护：未来实现应当限制单流消息数与响应大小（地址列表长度上限给出确切值），
  防止超长地址列表与高频查询撑爆服务端。

## 6. 兼容与版本

- draft 阶段（当前）：语义可以自由调整，不承诺任何兼容性。
- 转 stable 后：只能加法（新增可选字段/新错误码），不兼容变更必须升协议 ID 版本号
  （`/p2p-base/identify/1` → `/p2p-base/identify/2`），新旧 ID 并存一个过渡期，
  禁止原地修改既有 ID 语义（spec-charter.md §4.2）。
- 第三方兼容声明：本协议未升 stable 前，任何实现不得声明兼容 `/p2p-base/identify/1`。
- 探测方式：与其他协议一致，以「开流写协议 ID → 被关流」判定对端不支持，无预协商。

## 7. 测试向量

按 spec-charter.md §8 首波清单，本协议无专属向量集（registry 中 vectors 为空）。
定稿后应当引用以下通用向量文件（由向量轮并行交付，落地前按文件名引用，不另造向量）：

- `vectors/frame.json`：帧封装往返、1 MiB 上限、长度不符断流；
- `vectors/varint.json`：长度前缀 varint 边界值与溢出拒绝；
- `vectors/peer-id.json`：固定种子 → 公钥 → SHA-256 → base58 全链向量
  （供公钥字段与握手 PeerId 比对校验用）。

## 8. 实现状态与出处

- 常量登记（唯一事实源）：crates/p2p-relay/src/lib.rs:9（`proto_ids::IDENTIFY`）。
- 无 handler、无消息结构、无测试、无线上实例；registry 标注 impl="planned"、
  spec_status="draft"。
- 漂移登记：docs/design/wire-protocol.md §3.2 该行写「handler 随 S 装配接线」，系
  预期计划而非现状；实现状态以 registry（planned）与本文为准，文档侧不改（登记待
  对齐件轮刷新）。
