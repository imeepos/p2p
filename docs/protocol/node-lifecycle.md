# 节点发现与连接生命周期（node-lifecycle）

状态：v1。事实基线：crates/ 代码 + docs/design/wire-protocol.md；冲突以代码为准。
本文常量均标注出处（crate 相对路径:行号，为 2026-09-06 main 分支实况）。

## 1. 节点发现

### 1.1 mDNS 局域网发现（crates/p2p-discovery/src/mdns.rs）

- 服务类型固定 `_p2pbase._udp.local.`（:17，mdns-sd 要求带尾点格式）。
- 节点同时通告与浏览：通告周期默认 5s（announce_interval，:44），TTL 默认 15s
  （ttl_secs，:45）；远端以最近一次通告续期，过期由 1s 周期扫描判定下线（:20）。
- 实例名默认 `p2p-<PeerId 前 8 字符>`（:40），同机多实例需唯一。
- 通告携带 TXT 记录（mdns/txt.rs:7-9）：`peer` = base58 PeerId（必带），
  `quic` / `tcp` = 监听端口（可选）；地址由 mDNS 自动填充本机全部接口地址，
  对端按 地址集 + TXT 端口 还原 QUIC/TCP 传输地址集。
- 事件语义：发现（含地址与来源 mdns）、更新、过期下线；对端进程被杀时靠 TTL
  到期判定离线，最长滞后约一个 TTL 周期。

适用边界（必须知晓）：

- 仅同一二层局域网内有效；跨子网（经路由器）、跨 NAT、大多数虚拟机 NAT 不可达。
- mDNS 不是跨网发现方案：跨网用 rendezvous（1.2）；两者可并用。
- 防火墙需放行 UDP 5353 组播（实现方部署注意事项）。

### 1.2 rendezvous 跨网注册/查询（crates/p2p-discovery/src/rendezvous/）

传输：控制协议 `/p2p-base/rendezvous/1`，帧内 payload 为 protobuf
（Request/Response）。报文字段（messages.rs）：

| 报文 | 字段（protobuf tag） |
|---|---|
| AddrMsg | quic(1,bool)、ip(2,string)、port(3,uint32) |
| Register | namespace(1)、peer_id(2,bytes)、pubkey(3,bytes)、addrs(4,repeated AddrMsg)、ttl_secs(5,uint32)、sig(6,bytes)、issued_at(7,uint64,unix 秒) |
| Query | namespace(1)、peer_id(2,bytes，精确查号可选) |
| PeerEntry | peer_id(1,bytes)、addrs(2,repeated AddrMsg) |
| Response | error(1,string，空=成功)、peers(2,repeated PeerEntry) |
| Request | oneof: Register(1) / Query(2) |

签名验证（服务端三点全过才收，messages.rs:162-176 + wire-protocol.md 第 7.2 节）：

1. pubkey 推导出的 PeerId 与声称的 peer_id 一致（base58(SHA-256(pubkey))）。
2. sig 为对该节点 ed25519 公钥可验的签名，覆盖 SignedFields{namespace, peer_id,
   addrs, ttl_secs, issued_at} 的 protobuf 序列化字节——TTL 与签发时刻均入签名，
   杜绝篡改 TTL 或重放旧帧仍验签通过。
3. 时间新鲜：|now - issued_at| <= 300s（FRESH_TOLERANCE_SECS，messages.rs:152）。

服务端资源与取值约束（server.rs:22-32）：

| 约束 | 值 |
|---|---|
| namespace 长度 | 非空且 <= 64 字节（MAX_NAMESPACE_LEN） |
| 注册 TTL | 上限 3600s，超出截断（MAX_TTL_SECS；客户端默认申请 60s） |
| 每 namespace 节点数 | 上限 512（MAX_PEERS_PER_NAMESPACE） |
| 每连接注册限速 | 10 次/分钟（RATE_LIMIT_PER_MINUTE，令牌桶） |
| 每连接查询限速 | 120 次/分钟（RATE_LIMIT_QUERIES_PER_MINUTE） |
| 单次注册地址数 | 上限 32（MAX_ADDRS_PER_REGISTER）；任一地址非法整单拒绝 |

地址卫生（wire-protocol.md 第 7.2 节 E5）：公共策略 public_only 下，全不可路由
（loopback/链路本地）地址的注册整单拒绝；客户端查询侧独立剥离不可路由单条地址、
全不可路由的对端整体跳过；私网地址两端保留（同 NAT 直连合法）。

客户端节奏（client.rs:20-29,73）：注册每 20s 重发（兼作控制链路保活）；
查询启动期 5s 一轮共 2 轮，之后稳态 30s 一轮（附 +-20% 抖动防应答风暴同步）。
控制链路断开按指数退避重连：500ms 起、上限 30s、抖动 20%（reconnect.rs:6-9）。

### 1.3 地址缓存与 TTL

- 进程内地址缓存（AddrCache/MemCache，cache.rs）：put 覆盖旧条目并按给定 TTL
  续期；get/evict 只返回未过期条目，过期即清除。TTL 由写入方语义决定：
  mDNS 条目约 15s、rendezvous 注册条目为注册 TTL（默认 60s、上限 3600s）。
- 节点地址簿（crates/p2p-swarm/src/swarm/book.rs）按来源并集维护，只增不汰；
  rendezvous 查询到的地址同样入簿。第三方实现至少应做到：TTL 过期不作为
  唯一离线证据（配合 1.3 活性探测），死地址拨号失败要降权。

### 1.4 地址观测（简述）

NAT 内节点无法自知己身公网映射地址。本栈由 bootstrap 兼任观测反射口
（默认 3402/udp，crates/p2p-cli/src/cli.rs）：节点向反射口发包，从应答源地址
学到自身公网映射地址，随后将其并入 rendezvous 注册地址集。探测有界重试
（默认 3 次，首次失败后 500ms 起逐次翻倍封顶 16 倍；crates/p2p/src/lib.rs:66-69）。
第三方接入方在纯对称 NAT 后可跳过观测，直接依赖中继兜底。

## 2. 连接链路：直连 -> 打洞 -> 中继降级链

拨号入口只给目标 PeerId；地址取自地址簿。每一跳结果发 DialHop 事件
（Direct/Punch/Relay，crates/p2p-swarm/src/lib.rs:71-78），失败不静默降级。

### 2.1 直连跳（swarm/dial.rs + book.rs）

- 地址排序（book.rs:102-141）：mDNS 来源最优先 -> 与本机 mDNS 链路同网段
  （v4 /24、v6 /64）次之 -> rendezvous 全局地址 -> 其余（显式登记、loopback）殿后；
  同级内 QUIC 先于 TCP；对端地址与自身观测地址同公网前缀的 hairpin 候选同级殿后。
- 逐地址尝试：QUIC 握手超时 10s（crates/p2p-transport/src/quic.rs:25）、
  TCP 连接超时 5s（tcp.rs:18）；hairpin 候选统一短预算 2s（dial.rs:29），
  快速失败让位后续地址。单个地址失败继续下一个；TCP 入站 refused 不作为
  最终错误上抛。
- 全部地址失败：未配置中继则拨号失败（末次错误）；配置了中继进入 2.2。

### 2.2 打洞跳（swarm/degrade.rs + crates/p2p-relay/src/punch.rs + messages.rs:60-78）

1. 向中继预留电路 Reserve -> Reserved(circuit_id, load_permille)；电路预留 TTL
   120s（CIRCUIT_TTL，degrade.rs:21），对端未在期内接入即被中继回收。
2. 经中继交换打洞信令 PunchReq/PunchAck（字段：peer_id + addrs；中继转发时把
   peer_id 改写为实际发送方，接收方看到的就是对端真实身份）；信令有界重试
   5 次、间隔 400ms（SIGNAL_RETRIES/SIGNAL_RETRY_GAP，degrade.rs:27-31）。
3. 双方收到对端地址后同时向对方发包探测（单地址探测上限 5s，PROBE_TIMEOUT，
   degrade.rs:25）；探测成功即建立直连（QUIC 优先），Punch 跳完成。
4. 探测全失败：落入中继兜底（2.3），发 Punch 失败事件并继续，不静默。

### 2.3 中继跳（crates/p2p-relay/src/control.rs、circuit.rs、slots.rs）

1. 发起方接入自己预留的电路（Connect 携 circuit_id）；对端按信令里的 cid/ 项
   接入同一电路，两侧接入即配对。等待对端接入上限 10s（JOIN_TIMEOUT，degrade.rs:23）。
2. 中继只在 `/p2p-base/circuit/1` 流上桥接两侧密文字节，不解不存（relay lib.rs:3）；
   电路密文之上发起方仍须完成安全握手（同 wire-format.md 第 4 节）并校验
   期望 PeerId——中继是哑管道，信任边界仍在两端。
3. 服务端约束：电路默认 TTL 300s、上限 3600s（slots.rs:11-13）；每 Peer 链路 8、
   每 Peer 电路 32、全站链路 256、全站电路 1024（limits.rs:38-51）；超限回
   Reject(code=3)。水位经 Reserved/KeepAliveAck 的 load_permille（0..=1000）广播，
   客户端据此做多中继负载感知选路。

### 2.4 中继控制面错误码（messages.rs:12-23）

| code | 含义 |
|---|---|
| 1 | connect 引用的电路不存在 |
| 2 | 电路预留已过 TTL |
| 3 | 每 Peer 配额（链路/电路/带宽）超限 |
| 4 | 协议违规（首帧/状态机不合法；含未知新枚举变体） |
| 5 | 打洞目标当前无控制链路可转发 |
| 6 | 接入方不在电路允许名单 |
| 7 | 全站资源总量已打满 |

收到 Reject 即对应动作失败；状态机违规同时断流。

## 3. 断线重连行为

### 3.1 活性探测（swarm/ping.rs + lifecycle.rs:179-187）

- 已建连接按 10s 间隔走 `/p2p-base/ping/1` 探活（读一帧原样回一帧），
  单次探测超时 3s；连续失败判定连接死亡并发下线事件。
- 探测走协议层而非传输层 keepalive：QUIC/TCP/中继电路语义一致，
  且能发现对端进程挂起（半死连接）。

### 3.2 重连退避（swarm/lifecycle.rs + backoff.rs）

- 对已知 PeerId 的重连按指数退避：基准 1s、上限 60s，附抖动（lifecycle.rs:184-185）。
- 连接存活满 30s（reset_min_uptime，:187）后被判定健康，退避序列复位重计——
  避免对长期不稳定链路退避到永远等不到窗口。
- 拨号触发（业务 request/new_stream）与后台重连并存；退避只约束后台节奏。

### 3.3 发现/中继会话保活

- rendezvous：注册 20s 一发即保活（1.2）；控制链路断开按 500ms->30s 退避重连
  并自动重注册。
- 中继控制流：客户端 5s 间隔 KeepAlive、5s 超时（keepalive.rs:51-52）；
  服务端 45s 静默判死并回收该链路上未配对电路（:54）；已桥接电路空闲
  120s 由 TTL 清扫回收（:55，relay 控制流消失也触发回收）。

### 3.4 空闲连接回收（swarm/reclaim.rs:38-41）

- 空闲超 120s 的连接被后台回收（30s 扫描一轮），对端有在途使用时豁免；
  关闭原因事件化：idle / error / refused，排障可归因。

## 4. 超时/上限常量速查表

| 常量 | 值 | 出处 |
|---|---|---|
| mDNS 通告周期 | 5s | crates/p2p-discovery/src/mdns.rs:44 |
| mDNS 存活 TTL | 15s（1s 扫描判过期） | 同上 :45,:20 |
| rendezvous 注册间隔 | 20s | rendezvous/client.rs:20 |
| rendezvous 查询间隔 | 启动 5s x2 轮，稳态 30s | 同上 :25-29 |
| rendezvous 默认注册 TTL | 60s（服务端上限 3600s） | 同上 :67；server.rs:26 |
| rendezvous 新鲜度容差 | 300s | rendezvous/messages.rs:152 |
| rendezvous 重连退避 | 500ms -> 30s，抖动 20% | rendezvous/reconnect.rs:6-9 |
| 地址缓存 TTL | 随条目语义（mDNS 15s / 注册 TTL） | discovery/cache.rs:59-65 |
| QUIC 握手超时 | 10s | crates/p2p-transport/src/quic.rs:25 |
| QUIC 空闲超时 | 30s（keepalive 10s） | 同上 :23,:21 |
| TCP 连接超时 | 5s | crates/p2p-transport/src/tcp.rs:18 |
| Noise/TLS 握手超时 | 10s | crates/p2p-security/src/noise.rs:24 |
| hairpin 拨号预算 | 2s | crates/p2p-swarm/src/swarm/dial.rs:29 |
| 打洞信令重试 | 5 次 x 400ms | swarm/degrade.rs:27-31 |
| 打洞单地址探测超时 | 5s | 同上 :25 |
| 电路预留 TTL（拨号方） | 120s；对端接入等待 10s | 同上 :21,:23 |
| 电路默认/最大 TTL（服务端） | 300s / 3600s | crates/p2p-relay/src/slots.rs:11-13 |
| relay 限流 | 链路 8/Peer、电路 32/Peer、全站 256/1024、打洞 60 条/分 | crates/p2p-relay/src/limits.rs:38-51 |
| relay 保活 | 5s 间隔 / 5s 超时 / 服务端 45s 判死 | crates/p2p-relay/src/keepalive.rs:51-54 |
| 电路空闲回收 | 120s | 同上 :55 |
| 活性探测 | 10s 间隔 / 3s 超时 | crates/p2p-swarm/src/lifecycle.rs:181-182 |
| 重连退避 | 1s -> 60s，健康 30s 复位 | 同上 :184-187 |
| 空闲连接回收 | 120s 阈值 / 30s 扫描 | crates/p2p-swarm/src/swarm/reclaim.rs:38-41 |
| 单连接流上限 | 64 | crates/p2p-mux/src/lib.rs:73 |
