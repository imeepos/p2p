# /p2p-base/identify/1 规范

状态：stable；自 2026-09-01；归属 crates/p2p-swarm；符合性：Extended 项=全部章节
（identify 是补充性协议，节点可以不实现；未实现方收到本协议流时按 §4 关流即合规，
不影响其 Core 符合性）

## 1. 概览

- 解决什么问题：传输安全握手只互认 PeerId，不交换地址信息；mDNS 仅局域网有效，
  rendezvous 需要注册服务端。identify 提供点对点的「公钥 + 监听地址 + 观测地址 +
  软件版本」交换通道，服务于静态拨号对端、无发现基础设施等补充场景。
- 适用边界：补充性协议，不替代 mDNS/rendezvous；不承载身份认定（见下）。
- 与握手身份认定的关系：身份认定只来自握手（PeerId = base58(SHA-256(ed25519 公钥))，
  见 wire-format.md §4）。本协议一切自报字段（含公钥、地址）都不得作为身份证据，
  只能作为待验证线索；分工是「握手管身份、identify 管地址线索」。
- v1 范围：一问一答（一流一事务）。周期性地址推送、把 learned 地址接线进
  节点地址观测链路（design §7.2：identify → bootstrap 告知 → 打洞验证）属后续
  版本/后续卡，不在本版语义内。

## 2. 线格式

### 2.1 协议 ID 与帧

协议 ID 字符串 `/p2p-base/identify/1`，作为流首帧 payload 的 UTF-8 字节共 20 字节；
按帧封装（wire-format.md §6）完整首帧为 21 字节：

    14 2f 70 32 70 2d 62 61 73 65 2f 69 64 65 6e 74 69 66 79 2f 31

帧封装沿用 wire-format.md §6：无符号 varint 长度前缀 + payload，单帧上限
1 048 576 字节；varint 前缀最长 10 字节。之后每帧 payload 为一条完整 protobuf
消息（Request 或 Response），风格遵循控制面惯例（wire-protocol.md §7）。

### 2.2 Request 字段表

| tag | 字段 | 类型 | 取值域与上限 |
|---|---|---|---|
| 1 | protocol_version | uint32 | 必须 = 1；其他值触发 §4 VersionMismatch |

### 2.3 Response 字段表

| tag | 字段 | 类型 | 取值域与上限 |
|---|---|---|---|
| 1 | pubkey | bytes | 恰 32 字节，应答方 ed25519 公钥原始字节；其他长度即 §4 DecodeError |
| 2 | listen_addrs | repeated AddrMsg | 可为空（纯客户端节点合法）；条目形态见下 |
| 3 | observed_addr | optional AddrMsg | 服务端在本连接上观测到的请求端远端地址；观测不到（中继电路、无身份裸流）时缺省 |
| 4 | software | optional string | 形如 `p2p-base/0.1.0`；缺省合法 |

AddrMsg 与 rendezvous 的地址形态同构，字段只增不改：

| tag | 字段 | 类型 | 取值域与上限 |
|---|---|---|---|
| 1 | quic | bool | true=QUIC 地址，false=TCP 地址 |
| 2 | ip | string | 点分 IPv4 或 RFC 5952 IPv6 文本 |
| 3 | port | uint32 | 0..=65535 有效；越界条目接收端按 §5 丢弃 |

## 3. 时序与状态机

一问一答，单流单事务（风格对齐 ping）：

```
发起端                                应答端
  │ 开流 + 写协议 ID 首帧                │
  │ ──────────────────────────────▶     │ handler 接管（注册表路由）
  │ 写 Request（1 帧）                  │
  │ ──────────────────────────────▶     │ 校验 protocol_version
  │                                     │ 组装 Response（pubkey/listen/observed/software）
  │ 读 Response（1 帧）                 │ ◀──────────────
  │ pubkey 交叉核对（§4/§5）            │
  │ 关流                                │ 关流
```

- 每条流恰好一问一答：应答端写完 Response 后不得等待第二问；发起端读完
  Response 即事务结束。事务外多余帧按 §4 DecodeError 处理。
- 应答端必须在收到合法 Request 后应答或关流，不得静默挂流。
- 事务超时是发起端本地参数，本仓实现取 10 秒（§8）；协议本身不设线级超时，
  超时后发起端关流即视为事务失败，不重试（失败原因可观测，由调用方决策）。
- 应答侧 handler 注册：内置实现随 swarm 装配注入；registry 已含本协议 ID 时
  用户 handler 优先，内置实现不抢占。

## 4. 错误语义

显式信号 = 关流（不再写任何帧）；接收方观察到 EOF/流重置即知对端拒绝。
不得静默吞错后继续推进。

| 信号 | 触发 | 收发双方确切行为 |
|---|---|---|
| UnsupportedProtocol（关流） | 接收方无本协议 handler | 接收方立即关流并上报 UnsupportedProtocol（wire-format.md §8）；发起方必须视作「对端不支持」，不重试、不降级 |
| VersionMismatch（关流） | 应答端收到 protocol_version ≠ 1 | 应答端留告警日志后立即关流，不回任何帧（不猜测降级，design §5.2）；发起端表现为读响应时 EOF，按「对端版本不兼容」处理 |
| DecodeError（关流） | Request/Response protobuf 解码失败、Response.pubkey 长度 ≠ 32、事务外多余帧 | 解码失败方立即关流并留日志；对端表现为 EOF |
| PubkeyMismatch（断流） | 发起端将 Response.pubkey 推导 PeerId 与握手 PeerId 比对不一致 | 发起端必须按协议违规处理：丢弃响应、断流、留告警，不得采信任何自报字段；不向对端重发 |

## 5. 安全考量

- 自报地址不可信：接收端不得把对端自报的监听/观测地址当作已验证事实；地址
  入簿时应当降权，且按地址拨号必须携带期望 PeerId 校验（PeerMismatch 即断链，
  wire-format.md §4.4）。
- 身份只认握手：Response.pubkey 仅用于与握手推导 PeerId 交叉核对，两者不一致
  即 PubkeyMismatch（§4），防止身份嫁接。
- 观测地址语义：observed_addr 由应答端按本连接远端 socket 观测填写，是「我在
  本连接上看到的你的端点」；经中继电路的连接观测不到对端真实地址，必须缺省
  而非以中继地址冒充。
- 资源防护：单流单事务天然限流；单帧上限 1 048 576 字节约束响应大小；本版
  未另设 listen_addrs 条目数上限（应答端监听地址数量级为个位数，恶意超列表
  至多撑到帧上限即被拒）。不可解析条目（ip 非法、port > 65535）接收端丢弃
  该条目并留日志，不作整响应拒绝——identify 产出是线索而非准入凭证。
- 缓存：响应可缓存，TTL 必须 ≤ 60 秒，由调用方决定；本仓 v1 客户端不做缓存。
- 响应不落盘。

## 6. 兼容与版本

- stable 起：只能加法（新增可选字段/新错误码），不兼容变更必须升协议 ID 版本号
  （`/p2p-base/identify/1` → `/p2p-base/identify/2`），新旧 ID 并存一个过渡期，
  禁止原地修改既有 ID 语义（spec-charter.md §4.2）。
- 探测方式：与其他协议一致，以「开流写协议 ID → 被关流」判定对端不支持，无预协商。
- 旧版本容忍：字段只增不改，旧端收到带未知字段的帧必须忽略未知字段（protobuf
  语义天然满足）；protocol_version 是唯一的版本门。
- 第三方兼容声明：本页 stable 后，实现 §2 线格式与 §3/§4 行为即可声明兼容
  `/p2p-base/identify/1`。

## 7. 测试向量

本协议无专属向量集（registry 中 vectors 为空），依赖以下通用向量文件
（由向量轮交付，落地前按文件名引用，不另造向量）：

- `vectors/frame.json`：帧封装往返、1 MiB 上限、长度不符断流；
- `vectors/varint.json`：长度前缀 varint 边界值与溢出拒绝；
- `vectors/peer-id.json`：固定种子 → 公钥 → SHA-256 → base58 全链向量
  （供 pubkey 字段与握手 PeerId 比对校验用）。

## 8. 实现状态与出处

- 协议 ID 常量（唯一事实源）：crates/p2p-relay/src/lib.rs（`proto_ids::IDENTIFY`）。
- 线格式消息：crates/p2p-protocol/src/identify.rs（prost 手写 derive，
  `Request`/`Response`/`AddrMsg`，`PROTOCOL_VERSION = 1`）。
- 应答端 handler 与发起端客户端：crates/p2p-swarm/src/swarm/identify.rs
  （`IdentifyHandler` 随 `Swarm::assemble` 注入，用户 handler 优先；
  `Swarm::identify()` 为客户端入口，事务超时 10 秒；pubkey 交叉核对在
  `verify_response`）。
- 观测地址数据源：crates/p2p-mux/src/lib.rs `MuxControl::remote_endpoint()`
  （加法默认 None；QuicMux 返回 quinn `remote_address()`，YamuxMux 经
  `new_with_remote` 携带 TCP 对端地址）；swarm 在 accept/直连拨号入池时记入
  每 peer 观测表。中继电路 mux 未覆盖该方法，观测缺省（§5）。
- 测试：crates/p2p-itest/tests/identify_wave.rs（双节点互通 4 用例：基础交换/
  身份交叉核对失败断流/空 listen_addrs 合法性/版本不匹配拒绝）；p2p-protocol
  与 p2p-swarm 各含单元测试（消息往返、版本拒绝、核对、注入优先级）。
- 已知边界：同 peer 新旧连接交接瞬间（连接收敛竞态窗口），观测表取最新入池
  连接值，个别在途流的观测地址可能来自新连接——每 peer 单槽覆盖的既定语义，
  不影响直连常态。
- 漂移登记：无。本页与实现同轮交付（INTEROP IV2），docs/design/wire-protocol.md
  §3.2 状态行已同步。
