# /p2p-base/rendezvous/1 规范

状态：stable；自 2026-09-01；归属 crates/p2p-discovery；符合性：Core 项=注册/查询语义与签名验证；Extended 项=公共部署策略 public_only（服务端可以不启用）

## 1. 概览

- 解决什么问题：跨网（跨 NAT/跨子网）节点发现——节点向服务端签名注册自己的地址集，
  按 namespace 查询他人的地址集。mDNS 只覆盖同一二层局域网，rendezvous 是跨网互补
  方案，两者可以并用。
- 角色：服务端（公网注册节点）、客户端（同时注册自身并查询他人）。
- 与相邻协议的关系：查询到的地址进入对端地址簿后，连接建立走传输双路与降级链
  （node-lifecycle.md §2）；地址观测（反射口学公网映射地址）产出的地址应并入注册
  地址集。
- 领域模型：namespace（注册域，字符串）→ 节点条目（PeerId + 地址集 + TTL）。
- 默认 namespace：`p2p-base`（本仓实现常量，crates/p2p/src/assembly.rs:25
  DEFAULT_NAMESPACE）；查询"标准节点"必须使用该 namespace，其他 namespace 只承载
  显式注册到该域的条目。

## 2. 线格式

帧 payload = protobuf 消息（字段只增不改）；一帧一个消息，会话为多帧请求-应答。
全部整数 protobuf varint；bytes 为原始字节。

链路帧封装（本协议专用；先封帧再解 protobuf）：

- 首帧 = 协议 ID 帧，走 wire-format.md §6 varint 帧：payload 为
  `/p2p-base/rendezvous/1` 的 UTF-8 字节（22 字节），完整首帧 23 字节
  `16 2f70 32 70 2d 62 61 73 65 2f 72 65 6e 64 65 7a 76 6f 75 73 2f 31`。
- 其后每条消息帧 = **4 字节大端（u32be）无符号长度前缀 + protobuf payload**，
  一帧恰好一条 Request 或 Response；长度 = 该 protobuf 消息全长，上限
  1 048 576（1 MiB），超限读端立即链路级错误断开，不预读 payload。
- 消息帧不得使用 varint 前缀：服务端按 u32be 解读长度，varint 编码的长度字节会
  命中 frame too big 断开（GAP-1 实测，见 §8）。
- 逐字节样例：§7 `vectors/rendezvous-link-frame.json`（协议 ID 首帧、u32be 消息帧
  各一例 + 超限负样例）。

Request（oneof kind，tag 1=Register / tag 2=Query）：

Register（注册）字段表：

| tag | 字段 | 类型 | 语义与取值域 |
|---|---|---|---|
| 1 | namespace | string | 非空，UTF-8 字节长度 <= 64 |
| 2 | peer_id | bytes | 32 字节裸 SHA-256 摘要（非 base58 文本） |
| 3 | pubkey | bytes | 32 字节 ed25519 公钥原始字节 |
| 4 | addrs | repeated AddrMsg | 单注册 <= 32 条；可以为空（仅作在场标记） |
| 5 | ttl_secs | uint32 | 期望存活秒数；服务端截断到 <= 3600 |
| 6 | sig | bytes | 64 字节 ed25519 签名（覆盖域见下） |
| 7 | issued_at | uint64 | 签发时刻 unix 秒；须满足 \|服务端时钟 - issued_at\| <= 300 |

AddrMsg（地址三元组）：tag 1 `quic`（bool，true=QUIC/UDP，false=TCP）、
tag 2 `ip`（string，点分 IPv4 或 RFC5952 IPv6 文本）、tag 3 `port`（uint32，
必须 <= 65535，越界整单拒绝，不截断）。

Query（查询）：tag 1 `namespace`（string）、tag 2 `peer_id`（bytes；32 字节=精确
查号，空=查询整个 namespace）。

应答：Response tag 1 `error`（string，空串=成功）、tag 2 `peers`（repeated
PeerEntry）。PeerEntry：tag 1 `peer_id`（bytes 32）、tag 2 `addrs`
（repeated AddrMsg）。

签名覆盖域：`sig` 是对 SignedFields 的 protobuf 序列化字节的 ed25519 签名。
SignedFields 字段表：tag 1 `namespace`（string）、tag 2 `peer_id`（bytes）、
tag 3 `addrs`（repeated AddrMsg）、tag 4 `ttl_secs`（uint32）、tag 5 `issued_at`
（uint64）。`pubkey` 与 `sig` 本身不入签名（公钥用于验签）。TTL 与签发时刻均入
签名，篡改 TTL 或重放旧帧都验签必败。序列化字节黄金样例见 §7 向量。

## 3. 时序与状态机

会话：客户端在控制链路上发 Request，服务端对每个 Request 回恰好一个 Response
（一问一答，不交叉流水）。链路断开即会话结束。

半帧悬挂（读端确切语义）：链路读端无帧级计时器。前缀不足 4 字节、或前缀已读但
payload 未到齐时，读端不产出帧、不回任何字节、不断开，悬挂持续到宿主连接终止。
链路层仅有两个出口：发送方关流（半帧遇 EOF，链路报错结束）；声明长度 > 1 MiB
（读端立即报错断开）。回收兜底在连接层：整条连接完全静默达 QUIC 空闲超时 30s
即回收（crates/p2p-transport/src/quic.rs:23）；发送方在连接的其他流上保持流量则
连接继续存活，半帧链路不被计时回收（swarm 空闲回收阈值 120s + 30s 扫描，仅面向
无在途业务流的连接，crates/p2p-swarm/src/swarm/reclaim.rs:38,40）。因此实现必须
对每条链路请求施加整体超时，不得依赖对端回收半帧（参考节奏见本节客户端值）。

服务端行为：

1. 校验顺序：namespace 非空且 <= 64 字节 → 地址数 <= 32 → 签名三点全过
   （pubkey 推导 PeerId 与声称 peer_id 一致；sig 可验且覆盖 SignedFields 全字段；
   时间新鲜 |now - issued_at| <= 300）→ public_only 策略（若启用）→ 每 namespace
   容量。任一失败整单拒绝，不部分入库。
2. 入库 = 以 PeerId 为键覆盖旧条目，TTL 取 min(申请值, 3600) 秒，自入库时刻起算；
   覆盖已有 peer 不受容量限制，仅新增受限（每 namespace <= 512 节点）。
3. 查询：精确查号 O(1) 键读取；全量返回未过期条目；未知/非法 namespace 返回空结果，
   必须不创建任何服务端状态（防查询撑内存）；过期条目查询不可见并即时清除。

客户端参考节奏（本仓实现值，第三方可以自定但应当保守）：

- 注册每 20s 重发一次（兼作控制链路保活；须小于传输层空闲超时 30s）。
- 查询：启动期 5s x 2 轮，之后稳态 30s，附 ±20% 抖动防应答风暴同步。
- 链路断开按指数退避重连：500ms 起、上限 30s、抖动 20%；重连后自动重注册。

## 4. 错误语义

应用层失败统一表达为 `Response.error` 非空字符串。客户端必须只判空/非空，不得解析
具体文案（文案不构成兼容面）。当前服务端会返回的失败（登记为实现事实，便于排障）：

| error 文案 | 触发 |
|---|---|
| empty namespace | 注册/查询 namespace 为空 |
| namespace too long | 注册 namespace 超 64 字节 |
| addr count above 32 | 单注册地址数超 32 |
| bad signature or stale register | 签名三点任一失败或 \|now-issued_at\| > 300 |
| no routable addr | public_only 策略下全地址不可路由 |
| namespace peer limit reached | 新增时该 namespace 已满 512 节点 |
| register rate limit exceeded | 该连接注册超 10 次/分（令牌桶） |
| query rate limit exceeded | 该连接查询超 120 次/分（令牌桶） |
| missing request kind | Request 未装任何 oneof 变体（含未知新变体） |

链路层：protobuf 解码失败即链路级错误断开；读端 EOF（查询即断）是会话正常终结，
服务端不计为错误。所有拒绝都是整单拒绝，客户端应当修正后重新注册，不得期待部分接受。

## 5. 安全考量

- 防劫持签名：注册必须携带身份私钥签名，覆盖域为 SignedFields 全字段（§2）；
  服务端入库键取 pubkey 推导的 PeerId，不信任任何自报身份。缺签、错签、改字段
  重放（TTL/issued_at 入签）一律拒绝。
- 时间新鲜：±300s 容差构成重放窗口；部署必须保证 NTP 对时。窗口内重放等价于签名者
  为自己的地址续期（地址仍绑定其身份），危害有界。
- 服务端资源约束（确切值）：namespace 非空且 <= 64 字节；注册 TTL 截断 3600s；
  每 namespace 512 节点；每连接注册限速 10 次/分；每连接查询限速 120 次/分；
  单注册 <= 32 地址；任一地址非法（含端口 > 65535）整单拒绝。
- 地址卫生（信任域边界）：
  - 服务端 public_only 策略：全部地址都不可路由（loopback/链路本地）的注册整单
    拒绝——签名记录不可改写，不做部分剥离；默认宽松策略保留同机部署/单测的
    全 loopback 可发现性。
  - 客户端查询侧独立过滤：剥离单条不可路由地址，全不可路由的对端整体跳过、不入
    地址缓存。该过滤以「rendezvous 服务端在本机信任域之外」为前提；同机 rendezvous
    （bootstrap 全 loopback）时必须关闭，否则误伤同机可发现性。
  - 私网地址两端均保留：同一 NAT 内直连是合法用途。
- 端口越界等畸形输入整单拒绝，不得静默截断。

## 6. 兼容与版本

- protobuf 只增不改：新增字段/变体不改已有 tag 与类型；实现必须忽略未知字段
  （protobuf 标准语义）。
- 新增 Request oneof 变体时，旧服务端把未知变体解析为空 kind，回
  `missing request kind` 错误且连接不断——失败可见，不静默误解。
- 不兼容变更必须升协议 ID 版本号（`/p2p-base/rendezvous/2`），新旧并存过渡，
  禁止原地改义。
- 探测方式：无预协商；对端不支持表现为协议 ID 帧被关流（UnsupportedProtocol）。

## 7. 测试向量

按 spec-charter.md §8 首波清单，本协议适用（向量文件由向量轮并行交付，落地前按
文件名引用，不另造向量）：

- `vectors/rendezvous-register.json`：SignedFields protobuf 序列化字节 + 固定种子
  ed25519 签名（签名字节级黄金样例，翻任一字节必须验签失败）；
- `vectors/peer-id.json`：固定种子 → 公钥 → SHA-256 → base58 全链向量
  （pubkey 推导 PeerId 校验用）；
- `vectors/varint.json`：tag 头与 varint 整数编码边界（ttl_secs/issued_at/端口）。
- `vectors/rendezvous-link-frame.json`：链路帧封装逐字节样例——协议 ID 首帧
  （varint 帧）与 u32be 前缀消息帧各一例 + 声明长度超 1 MiB 负样例；消息帧的
  Register 字节与 `rendezvous-register.json` 黄金内容同源（§2 帧封装规则联用）。

## 8. 实现状态与出处

- crates/p2p-discovery/src/rendezvous/messages.rs：全部消息结构；signed_payload /
  verify_register（签名三点 + ±300s 新鲜度，FRESH_TOLERANCE_SECS=300）；AddrMsg
  解析（端口 > 65535 拒绝）。
- crates/p2p-discovery/src/rendezvous/server.rs：约束常量（MAX_NAMESPACE_LEN=64、
  MAX_TTL_SECS=3600、MAX_PEERS_PER_NAMESPACE=512、RATE_LIMIT_PER_MINUTE=10、
  RATE_LIMIT_QUERIES_PER_MINUTE=120、MAX_ADDRS_PER_REGISTER=32）与注册校验、
  查询（精确查号/全量快照缓存/未知 namespace 不扩表）、每连接限速循环。
- crates/p2p-discovery/src/rendezvous/client.rs：注册 20s、查询 5s x2 → 稳态 30s
  ±20% 抖动、strip_unroutable 信任域开关（默认开启，同机 rendezvous 置 false）。
- crates/p2p-discovery/src/rendezvous/reconnect.rs：500ms → 30s 退避（抖动 20%）。
- crates/p2p-discovery/src/rendezvous/link.rs：帧缝（一帧一消息，长度前缀封装）；
  真实帧缝实现在 crates/p2p/src/rendezvous.rs:213-217（LengthDelimitedCodec，
  4 字节大端长度前缀），帧上限 1 MiB = MAX_RENDEZVOUS_FRAME（:211）。
- 默认 namespace 常量：crates/p2p/src/assembly.rs:25（DEFAULT_NAMESPACE =
  "p2p-base"，同文件 :216 用于注册/查询配置）。
- lan-only 装配：crates/p2p/src/assembly.rs:126-133 清空 bootstrap 清单（不分公网
  私网回环），:195 输出 "bootstrap empty, rendezvous wiring skipped"——lan-only 下
  rendezvous 注册/查询接线整体跳过（quickstart.md §3 方式 B 同步登记）。
- 漂移登记（2026-09-11 W2 更正）：本页 §2 原未写消息帧前缀格式，与
  wire-format.md §6 一并被读作"消息帧 = varint 帧"；代码实况为 u32be 前缀
  （黑盒实测 GAP-1：varint 实现被服务端 frame too big 断开）。§2 已按代码写死，
  wire-format.md §6 已加适用范围声明。docs/design/wire-protocol.md §7.2 与
  docs/protocol/node-lifecycle.md §1.2 均与代码一致。
