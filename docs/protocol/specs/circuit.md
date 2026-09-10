# /p2p-base/circuit/1 规范

状态：stable；自 2026-09-01；归属 crates/p2p-relay；符合性：Core 项=全部章节（经中继兜底的唯一数据通道语义）

## 1. 概览

- 解决什么问题：直连与打洞都失败后，经中继把两侧已认证连接桥接成一条端到端字节
  通道。中继是哑管道：只搬运密文字节，不解密、不存储、不解释。
- 角色与流：电路两侧（owner 与 joiner）各自向中继开一条本协议流；中继把同一
  circuit_id 的第二条流与第一条配对，此后在两条流之间双向对拷原始字节。
- 与相邻协议的关系：circuit_id 由控制面 `/p2p-base/relay/1` 的 Reserve/Reserved
  发放，接入资格与槽位状态机同源于该页（specs/relay.md §3）；本协议只定义数据面。
- 信任边界：中继在密码学上不可信。电路之上必须完成端到端安全握手，信任锚仍在
  两侧端点。

## 2. 线格式

本协议流分两段，格式不同：

1. 首帧段（协议帧）：首帧必须是 Connect（RelayMsg protobuf 信封，`oneof kind`
   tag 3，字段 circuit_id(1, uint64)——与 relay 控制面同一消息族）。配对成功后，
   中继向两侧各回一帧 Bound（kind tag 4，字段 circuit_id(1)）。帧封装沿用
   wire-format.md §6（varint 长度前缀 + payload）。
2. 数据段（原始字节）：Bound 之后，流上其余字节为不透明密文，不再有任何帧边界、
   类型头或转义；中继把一侧读到的字节原样写到另一侧（双向对拷），直到一侧关闭。

密文的产生：两侧必须在电路之上自行完成传输安全握手（wire-format.md §4：QUIC/
TLS 或 Noise XX），密钥协商不得经过或依赖中继。发起方在电路上拨号时必须携带
期望 PeerId 校验。

约束：

- 首帧必须且只能是 Connect；其余帧（Reserve/Reject/KeepAlive 等）出现在本协议流
  上属控制面误用，由首帧分流层按协议违规处理（specs/relay.md §3）。
- Bound 之后中继不再解析任何字节；两侧应用必须自行定义密文层之上的消息边界。

## 3. 时序与状态机

全景时序（A=发起方/owner，B=接入方/joiner，R=中继；cid 为电路号）：

    A -> R  控制流 Reserve（allowed_joiner=B 或空）
    R -> A  Reserved(cid, load_permille)
    A <-> B 打洞信令 PunchReq/PunchAck 经 R 改写互换（可选路径；B 由此得知 cid）
    A -> R  本协议流首帧 Connect(cid)     # 第一条：R 收下停等，不回帧（Parked）
    B -> R  本协议流首帧 Connect(cid)     # 第二条：配对
    R -> A  Bound(cid)
    R -> B  Bound(cid)
    A <-> B 原始密文字节双向对拷（中继视角），直至一侧关闭或被回收

配对与停等语义：

- 同一 cid 的第一条 Connect 流被收下进入 Parked，中继不回任何帧；第二条到达即
  配对，两侧各收 Bound 后开始桥接。A、B 接入顺序可以互换，语义相同。
- Parked 停等没有显式协议超时，由电路 TTL 兜底（到期清扫直接关闭 Parked 流）；
  客户端参考接入等待为 10s。
- 接入资格裁决（cid 未知/过期/非白名单/配额满）在 Connect 时完成，语义见
  specs/relay.md §3-§4。

桥接期与拆除：

- 桥接中任一侧字节流 EOF 或错误即桥终；另一侧随之观察到 EOF。
- 双向持续静默满 120s（最近收发口径：任意方向出现数据即刷新）即判空闲，中继强制
  拆桥回收；两侧表现为流关闭，无显式错误帧。
- 桥终即槽位退役：配额立即回吐，不滞留至 Reserve TTL 清扫；此后到达的 Connect
  收 Reject(code=2)。
- 控制流关闭只回收未桥接（Reserved/Parked）电路，Bridged 桥不受影响。

## 4. 错误语义

| 信号 | 阶段 | 确切行为 |
|---|---|---|
| Reject(code=1) 未知电路 | Connect | 本流收 Reject 后关闭；多为 cid 记错或中继重启；控制流不受影响 |
| Reject(code=2) 电路过期 | Connect | TTL 已过或桥已退役；本流收 Reject 后关闭；客户端不得在同一 cid 重试 |
| Reject(code=3) 配额超限 | Connect | 接入方在途电路 32/Peer 已满；本流收 Reject 后关闭 |
| Reject(code=6) 非白名单 | Connect | 接入方非 owner 且非 allowed_joiner；本流收 Reject 后关闭 |
| Reject(code=4) 对端消失 | Bound | 一侧写 Bound 失败时，另一侧收 Reject(4, "circuit peer vanished") 后关闭 |
| 桥接中断链 | 数据段 | 出口带宽配额耗尽表现为写失败断链，无 Reject 帧；双方观察 EOF |
| 空闲回收 | 数据段 | 双向静默 120s 拆桥，无错误帧；双方观察 EOF |

客户端判定规则：收到 Bound 前收到 Reject 或 EOF 即接入失败，应当回到拨号降级链
继续（换中继或放弃），不得在同一 cid 上重发 Connect（只会再吃 code=1/2）；收到
Bound 即桥接成立，其后任何字节交换失败一律按链路失效处理并重走建链。

## 5. 安全考量

- 中继不可信（核心公理）：中继实现必须不解密、不缓存、不修改密文（仅两侧的
  Connect/Bound 控制帧除外）；客户端必须假设中继可观察密文长度与时序元数据——
  本协议不提供流量匿名。
- 端到端加密必须建立在电路之上：密钥协商不得依赖中继转发；电路上拨号必须校验
  期望 PeerId（PeerMismatch 即断）——这是防「中继替换对端」的唯一身份锚点。
- 接入面收敛：cid 由 CSPRNG 生成不可枚举，叠加 allowed_joiner 白名单与每 Peer
  电路配额（32）；带宽按每 Peer 出口令牌桶限速（1 MiB/s、桶 1 MiB 突发），超额
  断链不节流。
- 资源回收：空闲 120s 拆桥、桥终即退役配额，防死桥滞留占满全站 1024 电路。
- 半开桥：控制流静默判死（45s）与链路归零回收兜底，防失联侧滞留 Parked 流。

## 6. 兼容与版本

- 首帧消息族与 `/p2p-base/relay/1` 共用（RelayMsg），加法策略同 specs/relay.md §6：
  只增字段/变体，未知字段忽略，未知变体显式失败（Reject code=4）。
- 数据段无版本字段（原始密文字节）；版本语义完全由端到端握手层承载（TLS ALPN
  `p2p-base/1` 与 Noise 域串 `p2p-noise-xx-v1` 各含版本段，不匹配在握手期即失败）。
- 探测方式：无预协商；以 Connect 后收到 Reject/关流判定不可用。
- 不兼容变更必须升协议 ID 版本号（`/p2p-base/circuit/2`），禁止原地改义。

## 7. 测试向量

按 spec-charter.md §8 首波清单，本协议无专属向量集（registry 中 vectors 为空）。
适用通用向量文件（由向量轮并行交付，落地前按文件名引用，不另造向量）：

- `vectors/relay-messages.json`：Connect/Bound 的 oneof 黄金字节；
- `vectors/frame.json`：首帧段帧封装往返与长度不符断流；
- `vectors/varint.json`：tag 头与 circuit_id varint 编码边界。

## 8. 实现状态与出处

- crates/p2p-relay/src/circuit.rs：Connect 配对入口、Bound 下发（任一侧失败即取消
  桥接并对侧回 Reject(4)）、copy_bidirectional 密文桥接、空闲监管（最近收发口径
  的 120s 静默拆桥）、桥终退役与配额回吐。
- crates/p2p-relay/src/service.rs：本协议流首帧分流至电路面（Connect 之外即违规）。
- crates/p2p-relay/src/slots.rs：PendingStream/CircuitSlot/CircuitPhase
  （Reserved/Parked/Bridged）记账与到期清扫。
- crates/p2p-relay/src/keepalive.rs：idle_circuit_ttl 默认 120s。
- crates/p2p-relay/src/client.rs：RelayClient 接入（Connect）与 Bound 等待。
- 上层编排：crates/p2p-swarm/src/swarm/degrade.rs（接入等待 10s、桥上再握手拨号）。
- 协议 ID 唯一事实源：crates/p2p-relay/src/lib.rs:13（`proto_ids::CIRCUIT`）；
  同文件头注即「只桥接密文字节流、无法解密业务数据」契约。
- 漂移登记：docs/design/wire-protocol.md §3.2 将本协议实现出处写作
  「circuit.rs、state.rs」——state.rs 仅承载链路记账，电路槽位实际在 slots.rs；
  属文档粒度偏差而非语义漂移（登记待对齐件轮刷新）。
