# 字节级线协议规范 v1

状态：v1，与 2026-09-02 main 分支代码对齐。代码是最权威事实：本文每个常量都标注出处文件，
若与 `crates/` 实现冲突，以代码为准并修订本文。设计动机见 [p2p-base-design.md](p2p-base-design.md) §5/§6。

## 1. 层次模型

一条连接自外向内分四层，上层字节封装在下层字节流之内：

| 层 | QUIC 路径 | TCP 路径 |
|---|---|---|
| 传输 | quinn QUIC 数据报套（TLS1.3 内建加密） | tokio TcpStream（nodelay） |
| 安全 | TLS1.3 双向身份证书，随 QUIC 握手完成 | Noise XX 握手，之后进入 NoiseStream 记录层 |
| 复用 | QUIC 原生双向流 | yamux（拨号方固定 client 角色，tcp.rs:64/99） |
| 流语义 | 首帧协议 ID + 后续业务帧（本文 §4/§5） | 同左 |

复用层之上每条逻辑流独立使用 §3 的帧格式；一条流自始至终只承载一个协议。
两路产出统一为 `SecureConn { remote: PeerId, mux }`（crates/p2p-transport/src/lib.rs:28-32）。

## 2. 帧格式

所有流语义层的字节都按帧封装：**varint 无符号长度前缀 + 定长 payload**。

```
+------------------------------+======================================+
| len: unsigned varint, 1-10 B | payload: len 字节（len <= 1 MiB）    |
+------------------------------+======================================+
```

- varint 编码：LEB128，每字节低 7 位承载数据、最高位为后续标志，小端字节序分组；
  最多 10 字节，超出即 `varint overflow` 错误（crates/p2p-protocol/src/lib.rs:170-204）。
- 单帧上限 `MAX_FRAME_SIZE = 1 << 20`（1 048 576 字节），
  出处 crates/p2p-protocol/src/lib.rs:29；facade 配置默认同值（crates/p2p/src/lib.rs:23）。
- 超限行为：写侧先检查，超限直接报 `FrameTooLarge` 不写任何字节；读侧读完长度前缀后
  检查，超限即报错断流，不预读 payload（lib.rs:128-154）。长度与实际不符按 io 错误断流。
- 帧本身无类型字段、无版本字段；一帧的含义完全由它所在的流（协议 ID）与位置决定。

## 3. 协议 ID

### 3.1 语法

出处 crates/p2p-protocol/src/lib.rs:31-62（`ProtocolId::new` 校验规则）：

```
"/" <segment> ("/" <segment>)* "/" <digits>
```

- 必须以 `/` 开头；去掉开头 `/` 后按 `/` 切分至少得 2 段。
- 最后一段是**版本段**：非空纯 ASCII 数字（`1` 合法，`v1`、空串不合法）。
- 其余段：非空，仅小写字母、数字、`-`、`_`（大写与空格不合法）。
- 合法例：`/p2p-base/rendezvous/1`、`/myapp/chat/2`；非法例：`p2p-base/rendezvous/1`（无前导斜杠）、
  `/p2p-base/rendezvous`（无版本段）、`/p2p-base/rendezvous/v1`（版本段非纯数字）。

### 3.2 内置协议 ID 全表

五个内置控制协议 ID 在 crates/p2p-relay/src/lib.rs:9-13 统一登记（`proto_ids` 模块，
全底座唯一定义点，禁止重复定义）；职责划分源自 design §5.4：

| 协议 ID | 职责 | 当前实现状态 |
|---|---|---|
| `/p2p-base/identify/1` | 交换公钥、监听地址、观测地址 | 常量已登记；handler 随 S 装配接线 |
| `/p2p-base/ping/1` | 往返延迟探测、连通性保活 | 常量已登记；handler 随 S 装配接线 |
| `/p2p-base/rendezvous/1` | 签名注册/查询节点地址（带 TTL） | 客户端与注册表已实现（crates/p2p-discovery/src/rendezvous/，经 link 接缝对接传输） |
| `/p2p-base/relay/1` | 中继电路申请、打洞信令 | 已实现（crates/p2p-relay/src/control.rs） |
| `/p2p-base/circuit/1` | 中继电路数据桥接（密文透传） | 已实现（crates/p2p-relay/src/circuit.rs、state.rs） |
| `/repair/mcp/1` | repair-helper 与 repair-bridge 的 MCP stdio 字节隧道 | repair-bridge 哑泵（T20） |

业务协议 ID（如 `/myapp/chat/1`）与内置 ID 使用完全相同的注册与路由机制，无特权差别。

## 4. 新流开手顺序

出处 crates/p2p-protocol/src/handshake.rs（`open_with_protocol`/`dispatch_inbound`）：

```
发起方                                        接收方
  1. open_stream() 取一条逻辑流
  2. 首帧 = 协议 ID 的 UTF-8 字节             1. 读首帧，按 UTF-8 解析为协议 ID
     （普通帧封装，见 §2）                     2. 查 HandlerRegistry：
  3. flush 后即可写业务帧                        命中 -> 回调 handler，流交给 handler
  4. 之后每帧一个业务 payload                    未命中 -> UnsupportedProtocol，
                                                    关流并上抛，不猜测降级
```

- 协议 ID 帧的 payload 就是 ID 字符串本身的 UTF-8 字节（lib.rs:156-168）；
  非 UTF-8 或语法非法按 InvalidData 断流。
- 未注册协议返回 `ProtocolError::UnsupportedProtocol`（handshake.rs:23-35），
  语义为"关流并上报事件"，不做版本协商降级（design §5.2）。
- request-response 原语复用同一顺序：开流 -> 写协议 ID -> 请求帧 -> 读一帧回应 -> 关流
  （crates/p2p-protocol/src/request_response.rs:37-42），单个超时覆盖全程。

## 5. 安全握手

握手即认证：对端 PeerId 一律从密码学材料推导，任何"对端自报身份"都不得采信
（crates/p2p-security/src/lib.rs:1-5）。拨号时可指定期望 PeerId，不一致即 `PeerMismatch`。

### 5.1 QUIC：TLS1.3 + 身份证书

出处 crates/p2p-security/src/tls.rs、tls_cert.rs、tls_verify.rs、crates/p2p-transport/src/quic.rs。

- 双方各出一张自签证书（双向认证，rustls 层强制 CertificateVerify 验签），
  仅 TLS1.3（tls.rs:21/35）；ALPN 固定 `p2p-base/1`（tls.rs:14），协商不一致即握手失败。
- 证书携带私有扩展 `OID 1.3.6.1.4.1.59015.1`（tls_cert.rs:18），内容为原始 32 字节
  ed25519 公钥；校验规则：只认带此扩展的自签证书，不查 CA；且证书 SPKI 原始公钥必须
  与扩展内容逐字节一致（tls_cert.rs:90-97），扩展声明身份、SPKI 承担验签。
- 对端 PeerId = 从对方证书扩展取公钥再按 §6 推导（`peer_id_from_cert`，tls_cert.rs:102-105）。
- SNI 使用固定占位 `p2p-base`，不做域名校验（quic.rs:18）；QUIC keepalive 10 s（quic.rs:19）。
- 单连接双向流上限 64：quinn 传输参数与复用层信号量双重防护（quic.rs:21-28、mux lib.rs:36）。

### 5.2 TCP：Noise XX

出处 crates/p2p-security/src/noise.rs。

- 参数串 `Noise_XX_25519_ChaChaPoly_SHA256`（noise.rs:19）；XX 模式双方交换静态钥，无明文阶段。
- 噪声握手帧与传输态记录同格式：`u16be(len) || ciphertext`（noise.rs:147-176）。
- 三条消息流（发起方视角）：msg1 发 `e`（空 payload）；msg2 收 `e, s, encrypt(payload)`；
  msg3 发 `s, encrypt(payload)`。
- 身份 payload 定长 96 字节 = 32 字节 ed25519 公钥 || 64 字节 ed25519 签名（noise.rs:105-115）；
  签名内容为域分隔串 `p2p-noise-xx-v1`（noise.rs:21）拼接本端 X25519 静态公钥。
  长度不符或验签失败即 `IdentityUnverified`（noise.rs:117-135）。
- X25519 静态钥由同一 ed25519 身份派生：私钥 = SHA512(种子) 前 32 字节做 RFC 7748 clamp；
  公钥 = ed25519 公钥转蒙哥马利坐标（noise.rs:76-92）。签名因此把 ed25519 身份与
  Noise 静态钥绑定，持有自己静态钥的同时无法冒充他人身份。
- 对端 PeerId 从 payload 公钥推导（不采信任何字符串字段）；随后进入传输态，
  密文记录层持续使用 `u16be(len) || ciphertext` 格式，其上再跑 yamux。

## 6. PeerId 推导

出处 crates/p2p-identity/src/lib.rs:13-41。

```
PeerId = base58( SHA-256( ed25519 公钥原始 32 字节 ) )
```

- 内部为定长 32 字节（SHA-256 摘要），展示与拨号配置用 base58 字符串（bs58 编码）。
- 密钥即身份：Ed25519 种子 32 字节，签名输出 64 字节（lib.rs:47-82）；签名静态方法
  `Keypair::verify` 供 rendezvous 注册校验等无密钥方使用。
- 种子落盘为裸 32 字节文件，权限 0600，加载时收紧宽松权限、长度不符即报错不静默重建
  （crates/p2p-identity/src/seed.rs）。
- 备注：design 文档 §6 写作 `sha256(protobuf(公钥))`，实现为对原始 32 字节公钥直接取哈希，
  以实现为准（与 libp2p 的 multihash 封装也不同，不自增前缀）。

## 7. 控制面编码

控制面（发现/中继）payload 为 protobuf（prost 手写 derive，无 protoc），字段只增不改。

### 7.1 relay 控制面（/p2p-base/relay/1）

出处 crates/p2p-relay/src/messages.rs。帧 payload = RelayMsg protobuf 信封，
`oneof kind` tag 1-9：Reserve(1)/Reserved(2)/Connect(3)/Bound(4)/PunchReq(5)/PunchAck(6)/
Reject(7)/KeepAlive(8)/KeepAliveAck(9)。

- 控制流首帧必须是 Reserve，接入流首帧必须是 Connect；服务端按首帧分流（service.rs:1-3）。
- 状态机违规回 `Reject` 并断流；错误码（messages.rs:12-23）：1 未知电路、2 电路过期、
  3 每 Peer 配额超限、4 协议违规、5 打洞目标不在场、6 接入方不在允许名单、7 全站资源打满。
- 负载水位广播：`Reserved` = circuit_id(1) + load_permille(2)，`KeepAliveAck` =
  load_permille(1)；permille 0..=1000，取链路/电路/限速桶三资源占用率最大
  （service.rs `load_permille`，瓶颈口径）。客户端据此做负载感知的中继选择；
  旧端未知字段自动忽略（字段只增不改）。
- `PunchReq`/`PunchAck` 即打洞信令（类 DCUtR）：peer_id 字段指明目的地，relay 转发时
  改写为实际发送方，接收方看到的就是对端真实身份（messages.rs:3-5）。
- 电路建好后 `/p2p-base/circuit/1` 流上只搬运密文字节，relay 不解不存（lib.rs:3）。

### 7.2 rendezvous 控制面（/p2p-base/rendezvous/1）

出处 crates/p2p-discovery/src/rendezvous/messages.rs。帧 payload = Request/Response protobuf。

- Request `oneof` tag 1-2：Register(1)/Query(2)；Response 携带 error 字符串与 PeerEntry 列表。
- Register 字段：namespace(1)、peer_id(2)、pubkey(3)、addrs(4, repeated)、ttl_secs(5)、
  sig(6)、issued_at(7, unix 秒)。
- 防劫持签名（messages.rs）：sig = ed25519 签名，覆盖 `SignedFields{namespace, peer_id,
  addrs, ttl_secs, issued_at}` 的 protobuf 序列化字节；服务端校验三点——pubkey 推导出的
  PeerId 与声称的 peer_id 一致、签名有效、时间新鲜（|now - issued_at| <= 300s）。缺一即拒绝。
  TTL 与注册时刻均入签名，杜绝"篡改 TTL 或重放旧帧仍验签通过"（安全审查 H1）。
- 服务端资源与取值约束：namespace 非空且 <=64 字节、TTL 截断到 3600s、每 namespace peer
  数上限 512、每连接注册限速 10 次/分（令牌桶）；任一地址解析失败（含端口 >65535）即
  整单拒绝，不静默丢弃（M1/L2）。
- 地址卫生策略（E5，2026-09-03）：loopback/link-local 地址跨网不可拨。服务端公共策略
  （registry public_only，CLI bootstrap 默认开启、`--allow-private` 退出）对全不可路由
  地址的注册整单拒绝——签名记录不可改写，不做部分剥离；空地址注册为存量兼容语义保留。
  默认宽松策略保留同机部署/单测的全 loopback 可发现性。客户端查询侧独立过滤：剥离
  不可路由单条地址，全不可路由的对端整体跳过，不入地址缓存；过滤以信任域为界——
  rendezvous 本体在同机（bootstrap 全 loopback）时关闭，保住同机可发现性。私网地址
  两端均保留（同 NAT 直连合法用途）。
- 地址编码 AddrMsg：quic(1, bool)、ip(2, string)、port(3, uint32)。

### 7.3 chunked transfer（大 payload 分帧）

出处 crates/p2p-protocol/src/chunked.rs。用于单帧装不下的整条消息，仍走 §2 帧封装，
帧 payload 首字节为类型头：

| 类型头 | 值 | 语义 |
|---|---|---|
| FRAME_SINGLE | 0x00 | 整条消息一帧装下，到此结束 |
| FRAME_CHUNK | 0x01 | 中间分片，后随更多帧 |
| FRAME_END | 0x02 | 最后一个分片，读端收到即重组完成 |

- 每帧数据部分上限 `CHUNK_DATA_SIZE = MAX_FRAME_SIZE - 1 = 1 048 575` 字节（chunked.rs:22）。
- 重组总大小防御性上限 `MAX_MESSAGE_SIZE = 64 << 20`（64 MiB，chunked.rs:24），超限报
  `MessageTooLarge`；类型序非法（如分片中途出现 SINGLE）报 InvalidData。

## 8. 版本与演进策略

1. **帧封装无版本字段**：语义完全由流首帧的协议 ID 决定，帧格式本身保持稳定。
2. **加字段不破坏**：控制面 protobuf 只新增字段/变体、不改已有 tag 与类型；
   prost 对未知字段按 protobuf 语义跳过，旧实现可继续解析新消息。
3. **不兼容变更升协议 ID 版本号**：`/x/y/1` 不兼容地变成 `/x/y/2`，新旧 ID 在
   HandlerRegistry 中并存路由，由发起方选择；禁止原地改既有 ID 的语义。
4. **未知协议显式失败**：未注册协议关流上抛 `UnsupportedProtocol`，不猜测降级（§4）。
5. **控制面枚举加变体的失败可见性**：新 kind 发给旧服务端时，未知 oneof 载荷解析为空，
   服务端按协议违规回 `Reject(code=4)` 并断流（control.rs:53-57）——不会静默误解。
6. **传输层演进走 ALPN 与签名域**：QUIC ALPN `p2p-base/1` 与 Noise 域串
   `p2p-noise-xx-v1` 各含版本段；破坏性传输变更升位后，版本不匹配在握手期即失败，
   不会进入半兼容连接。

### 8.1 业务协议登记：/im/chat/1（IM 聊天）

出处 crates/p2p-chat/src/wire.rs。好友间 1:1 私聊协议（design im-chat-design.md §3），
一条出站流 = 一个消息事务（send-once）：开流 → 写信封帧 → 有附件再写媒体块帧 →
读 ACK → 关流。帧封装复用 §2（varint 长度前缀 + payload，≤1 MiB/帧），
帧 payload 首字节为类型头（与 chunked §7.3 同风格）：

| 类型头 | 值 | 载荷 |
|---|---|---|
| ENVELOPE | 0x01 | 信封 JSON（字段见下，单帧） |
| MEDIA_BEGIN | 0x02 | 媒体头 JSON：{len, name, mime, kind}（单帧） |
| MEDIA_CHUNK | 0x03 | 原始附件分片（每帧 ≤ 1 MiB - 1 字节，chunked 规则） |
| ACK | 0x04 | JSON：{id, ok, reason?} —— 对端确认已收完整消息 |

- 信封 JSON 字段：id（UUID，发端生成）、peer（线上 = 发端自身 PeerId，base58；
  底座 handler 拿不到对端 PeerId，收端据此落盘 messages/<peer>.jsonl）、
  sender（发端恒为 me；收端校验非 me 即伪装断流）、kind
  （text/image/audio/video/file）、tsMs（发端本地毫秒时间戳）、text?
  （kind=text 时，trim 后 1..=2000 字符）、media?（{name, mime, size}）；
  path/status 为本地字段不跨网。
- 时序：ENVELOPE →（可选 MEDIA_BEGIN → MEDIA_CHUNK×n）→ 对端 ACK。
  任意帧校验失败（帧超上限/类型序非法/信封缺字段或校验不过/媒体长度不一致）→
  断流并留告警日志，发端收失败即消息落 failed 态。
- 单条消息（含附件原始字节）≤ 64 MiB（对齐 chunked.rs MAX_MESSAGE_SIZE），
  超限发送前拒绝、入站断流；附件 MIME 按 kind 白名单校验（image/audio/video
  前缀匹配，其余归 file），不匹配即 Err/断流，不猜测不降级。
- 幂等：收端按消息 id 去重，重复投递仅回 ACK 不重复落盘（重发安全）。

## 9. 常量速查表

| 常量 | 值 | 出处 |
|---|---|---|
| MAX_FRAME_SIZE | 1 048 576（1 MiB） | crates/p2p-protocol/src/lib.rs:29 |
| varint 长度前缀上限 | 10 字节 | crates/p2p-protocol/src/lib.rs:188-204 |
| FRAME_SINGLE/CHUNK/END | 0x00 / 0x01 / 0x02 | crates/p2p-protocol/src/chunked.rs:17-19 |
| CHUNK_DATA_SIZE | 1 048 575 | crates/p2p-protocol/src/chunked.rs:22 |
| MAX_MESSAGE_SIZE | 67 108 864（64 MiB） | crates/p2p-protocol/src/chunked.rs:24 |
| QUIC_ALPN | `p2p-base/1` | crates/p2p-security/src/tls.rs:14 |
| 身份扩展 OID | 1.3.6.1.4.1.59015.1 | crates/p2p-security/src/tls_cert.rs:18 |
| NOISE_PATTERN | `Noise_XX_25519_ChaChaPoly_SHA256` | crates/p2p-security/src/noise.rs:19 |
| Noise 签名域 | `p2p-noise-xx-v1` | crates/p2p-security/src/noise.rs:21 |
| Noise 身份 payload | 96 字节（32 公钥 + 64 签名） | crates/p2p-security/src/noise.rs:105-135 |
| Noise 记录帧 | u16be 长度 + 密文 | crates/p2p-security/src/noise.rs:147-176 |
| MAX_STREAMS_PER_CONN | 64 | crates/p2p-mux/src/lib.rs:36 |
| QUIC SNI 占位 | `p2p-base` | crates/p2p-transport/src/quic.rs:18 |
| QUIC keepalive | 10 s | crates/p2p-transport/src/quic.rs:19 |
| mDNS 服务类型 | `_p2pbase._udp.local` | crates/p2p-discovery/src/mdns.rs:16 |
| 内置协议 ID（5 个） | 见 §3.2 全表 | crates/p2p-relay/src/lib.rs:9-13 |
