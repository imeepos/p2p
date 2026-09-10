# /p2p-base/relay/1 规范

状态：stable；自 2026-09-01；归属 crates/p2p-relay；符合性：Core 项=电路申请/接入/错误码语义；Extended 项=打洞信令转发（纯客户端对端可以不部署中继服务端）

## 1. 概览

- 解决什么问题：直连不可达时的两条兜底路径——(a) 申请经中继的电路（数据面走
  `/p2p-base/circuit/1`，另页规范）；(b) 借中继交换打洞信令建立 P2P 直连（类 DCUtR）。
- 角色：服务端（公网节点兼任，如 bootstrap）；客户端（申请电路、发信令、保活）。
- 双流模型：客户端到服务端的两类流共用本协议 ID，服务端按首帧分流——首帧
  Reserve = 控制流（一客户端至多一条，后到覆盖登记），首帧 Connect = 接入流
  （每电路每侧一条，接入后交给 circuit 协议语义）。
- 与相邻协议的关系：电路数据面见 specs/circuit.md；客户端侧编排（何时 Reserve、
  何时发信令）属拨号降级链（node-lifecycle.md §2.2-2.3），本文只定义协议行为。

## 2. 线格式

帧 payload = RelayMsg protobuf 信封：`oneof kind`，tag 1-9，一帧恰装一个子消息。
字段只增不改；整数 protobuf varint；PeerId 为 base58 文本；打洞地址为 `host:port`
文本。

| kind tag | 消息 | 字段（tag、类型、取值域） |
|---|---|---|
| 1 | Reserve | ttl_secs(1, uint64，0=用服务端缺省 300，上限截断 3600)；allowed_joiner(2, string，PeerId base58 文本；空=仅 reserve 者可接入，非空=仅 owner 与该 PeerId 可接入) |
| 2 | Reserved | circuit_id(1, uint64，服务端 CSPRNG 生成的非零值)；load_permille(2, uint32，0..=1000，发放时刻负载水位) |
| 3 | Connect | circuit_id(1, uint64) |
| 4 | Bound | circuit_id(1, uint64)；收到即本流后续字节与对端互通 |
| 5 | PunchReq | peer_id(1, string，目的地；中继转发时改写为实际发送方)；addrs(2, repeated string，发送方观测到的自身地址) |
| 6 | PunchAck | peer_id(1, string，同上改写规则)；addrs(2, repeated string) |
| 7 | Reject | code(1, uint32，1-7，见 §4)；message(2, string，诊断文案，不构成兼容面) |
| 8 | KeepAlive | 无字段 |
| 9 | KeepAliveAck | load_permille(1, uint32，0..=1000) |

负载水位口径：load_permille 取「链路占用、电路占用、限速桶占用」三类资源占用率的
最大值（瓶颈口径，剩余服务能力由最弱资源决定）；0=空闲，1000=打满。客户端可以
据此做多中继负载感知选路。

## 3. 时序与状态机

控制流（客户端 -> 服务端，一条长驻流）：

1. 开流后首帧必须 Reserve；服务端裁决后回 Reserved（成功，携 circuit_id 与水位）
   或 Reject（失败，控制流不断）。
2. 其后合法上行帧：Reserve（可再申请）、PunchReq、PunchAck、KeepAlive。出现
   Reserved/Connect/Bound/Reject/KeepAliveAck 等其他帧即协议违规：服务端回
   Reject(code=4) 并断流。
3. KeepAlive：客户端周期发送，服务端必须即时回 KeepAliveAck（捎带当前水位）。
   参考节奏：客户端 5s 间隔、单次往返超时 5s、连续 3 次无应答判中继失联（关控制流
   写半并上抛事件）；服务端 45s 收不到任何帧即按客户端失联清理（断流并回收该连接
   名下未桥接电路）。
4. 控制流存亡即注册载体存亡：控制流关闭时，服务端回收该控制流发放且尚未桥接的电路
   （Reserved/Parked 态），已桥接（Bridged）电路不受影响。

电路槽位状态机（服务端记账，cid 为键）：

    Reserved（Reserve 发放）-> Parked（第一条 Connect 收下，不回帧）
    -> Bridged（第二条 Connect 到达：两侧各回 Bound，开始桥接）-> 退役（桥终即退）

- TTL：Reserve 申请值截断到 [缺省 300s，上限 3600s]；Reserved/Parked 到期由服务端
  1s 周期清扫回收（Parked 中被丢弃的一侧流直接关闭，无 Reject 帧）。
- 接入流首帧必须 Connect：未知 cid → Reject(1)；过期 → Reject(2)；不在允许名单 →
  Reject(6)；接入方配额满 → Reject(3)；通过则 Parked 或配对。
- 桥接结束后迟到 Connect 收 Reject(2)。

打洞信令时序（A、B 两侧均有控制流）：

1. A 发 PunchReq{peer_id=B, addrs=A 观测地址}；
2. 中继校验 B 在场（有控制链路），把 peer_id 改写为实际发送方 A 后转发给 B；
3. B 回 PunchAck{peer_id=A, addrs=B 观测地址}，中继改写为 B 后转发给 A；
4. 双方即获得对端经中继认证的地址，可同时发包探测建直连（探测行为本身不在本协议内）。

改写规则（防伪造核心）：peer_id 字段在转发时一律被中继替换为该控制流认证的实际
发送方 PeerId；接收方看到的 peer_id 即对端真实身份，发送方填什么都会被覆盖。
信令限速：每控制流 60 条/分（令牌桶），超限回 Reject(code=4) 但不断流；目的地
无控制链路时回请求方 Reject(code=5)，双侧控制流不断。

客户端参考节奏（本仓拨号降级链值）：Reserve 申请 TTL 120s；等待对端接入上限 10s；
信令重试 5 次、间隔 400ms。

## 4. 错误语义

错误码全表（Reject.code）与确切行为：

| code | 语义 | 触发 | 确切行为 |
|---|---|---|---|
| 1 | 未知电路 | 接入流 Connect 引用不存在的 cid | 该接入流收 Reject(1) 后关闭；控制流不受影响 |
| 2 | 电路过期 | 预留 TTL 已过，或桥已退役后的迟到 Connect | 该接入流收 Reject(2) 后关闭 |
| 3 | 每 Peer 配额超限 | 链路 8/Peer、电路 32/Peer 超限；全站链路 256 打满时新链路同样以 code=3 拒 | Reserve/Connect 收 Reject(3)；超限链路对每条新流回 Reject(3) 后关闭（有界：至多 64 条、空闲 10s 放弃）；出口带宽超限不回帧，表现为桥接中断链 |
| 4 | 协议违规 | 首帧非 Reserve/Connect；控制流上出现非法帧；未知 oneof 变体（解析为空）；打洞信令限速 | 首帧/控制流违规：回 Reject(4) 后断流；信令限速：仅回 Reject(4) 不断流 |
| 5 | 打洞目标不在场 | PunchReq/PunchAck 目的地当前无控制链路 | 请求方收 Reject(5)，双侧控制流不断 |
| 6 | 接入方不在允许名单 | Connect 者非 owner 且非 allowed_joiner | 该接入流收 Reject(6) 后关闭 |
| 7 | 全站资源打满 | Reserve 时全站电路 1024 已满 | 控制流收 Reject(7)，连接不断，可稍后重试或换中继 |

Reject 的接收语义：控制流上的 Reject 是对应动作的失败应答（合法帧），接收方不得
因收到 Reject 而断控制流；接入流上的 Reject 即本流失败。`Reject.message` 文案仅供
排障，实现不得解析。收到 Reject 的客户端应当把该中继标记为不可用并按上层策略
换路，不得在同一 cid 上重试。

## 5. 安全考量

- 电路号不可枚举：cid 由 CSPRNG 生成的非零 u64，防第三方猜号接入。
- 接入白名单：allowed_joiner 空 = 仅 owner 可接入；非空 = owner 与指定 PeerId。
  接入方身份取自传输握手推导，不得采信 Connect 载荷之外的声称。
- 伪造防护：打洞信令的 peer_id 转发改写规则（§3）使「目的地寻址」与「来源标识」
  都锚定在中继认证的链路身份上，客户端无法借中继伪造他人身份发信令。
- 抗滥用配额（默认值，服务端可调但必须设）：每 Peer 链路 8、电路 32、出口带宽
  1 MiB/s（桶容量 1 MiB 突发，超额断链不节流）；全站链路 256、电路 1024、带宽桶
  256（桶表满时新 Peer 共享降级桶）；打洞信令 60 条/分/控制流。每 Peer 粒度可被
  Sybil 身份稀释，故必须叠加全站总量上限。
- 半开连接：服务端 45s 静默判死，防失联客户端长期占簿。
- 本协议不保护数据面机密性：电路数据面的信任边界见 specs/circuit.md §5。

## 6. 兼容与版本

- oneof tag 只增（1-9 已占用）：新变体追加新 tag；实现必须忽略未知字段。
- 未知变体的失败可见性：新变体发给旧服务端时，未知 oneof 载荷解析为空，服务端按
  协议违规回 Reject(code=4) 并断流——不会静默误解；旧端发给新端同理。这是控制面
  枚举演进的承诺：任何一端遇到不认识的消息都必须显式失败。
- 水位字段属可选信息：不识别 load_permille 的旧客户端可以忽略，不影响互通。
- 不兼容变更必须升协议 ID 版本号（`/p2p-base/relay/2`），禁止原地改义。

## 7. 测试向量

按 spec-charter.md §8 首波清单，本协议适用（向量文件由向量轮并行交付，落地前按
文件名引用，不另造向量）：

- `vectors/relay-messages.json`：RelayMsg oneof tag 1-9 黄金字节（含错误码表）；
- `vectors/frame.json`：帧封装往返、1 MiB 上限、长度不符断流；
- `vectors/varint.json`：tag 头与 varint 整数编码边界。

## 8. 实现状态与出处

- crates/p2p-relay/src/messages.rs：9 消息结构与错误码 1-7 唯一定义（errcode 模块）。
- crates/p2p-relay/src/service.rs：链路接收与配额拒（超限链路有界回拒：至多 64 条、
  空闲 10s）、首帧分流（Reserve=控制 / Connect=电路 / 其余=违规）。
- crates/p2p-relay/src/control.rs：控制循环（45s 静默判死）、Reserve 裁决、Punch
  转发（peer_id 改写）与限速（60 条/分）。
- crates/p2p-relay/src/slots.rs：槽位状态机（Reserved/Parked/Bridged/退役）、TTL
  缺省 300s 上限 3600s、CSPRNG cid、配对裁决、到期清扫。
- crates/p2p-relay/src/state.rs：链路记账、控制流登记（后到覆盖）与移除保护。
- crates/p2p-relay/src/limits.rs：配额默认值（8/32/1 MiB、256/1024/256、60 条/分）。
- crates/p2p-relay/src/keepalive.rs：客户端 5s/5s/3 次、服务端静默 45s、桥接空闲
  120s 参数与保活任务。
- crates/p2p-relay/src/load.rs + service.rs（load_permille）：水位瓶颈口径。
- crates/p2p-relay/src/client.rs：客户端（RelayClient，事件 PunchReq/PunchAck/
  ControlClosed）。
- 上层编排：crates/p2p-swarm/src/swarm/degrade.rs（Reserve 120s、接入等待 10s、
  信令 5 次 x 400ms）。
- 协议 ID 唯一事实源：crates/p2p-relay/src/lib.rs:12（`proto_ids::RELAY`）。
- 漂移登记：无已知语义漂移；docs/design/wire-protocol.md §7.1 与
  docs/protocol/node-lifecycle.md §2.3-2.4 与代码一致。
