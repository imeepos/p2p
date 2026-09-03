# P2P 连接生命周期调研：连接维持、在线判定与中继健康（E6-R1）

调研会话：E6-R1（连接稳定性轮，为 E7+ 决策提供依据）。检索日期：2026-09-03。
所有事实来自文中链接的公开规格/官方文档/源码；本仓库现状仅引用文件路径，不粘贴源码。

## 1. libp2p

连接生命周期由 Swarm 统一管理。rust-libp2p 默认在连接空闲 10 秒后将其关闭
（`Swarm::Config::with_idle_connection_timeout`，默认值由 PR #4967 固化为 10s），
但"in use"（有活跃流或被 keep_alive 语义持有）的连接自动豁免；需要长期钉住连接时
挂载 keep_alive 行为/handler。连接复用：go-libp2p BasicHost 开流使用 WithNoDial
（"already dialed"），优先复用既有连接而非重新拨号。重连退避是一等配置：
js-libp2p 对打 KEEP_ALIVE 标签的对端，断开后以 reconnectRetryInterval 为基、
按 reconnectBackoffFactor 倍增，重试 reconnectRetries 次；go-libp2p 的
BackoffConnector 以 LRU 记录每 peer 的下次可拨时间（nextTry = now + Delay()），
指数策略下处于退避期的 peer 直接拒绝再拨。

| 维度 | 做法 |
|---|---|
| 保活方式 | keep_alive 行为钉住连接；有活跃流即视为 in use，免于空闲回收 |
| 探活间隔 | 无应用层 ping；空闲超时默认 10s（可配） |
| 离线判定 | 空闲超时或连接错误导致关闭，经事件外抛给上层 |
| 重连策略 | js：interval × factor 指数退避 + 次数上限；go：BackoffConnector 退避缓存 |
| 中继健康检测 | 委托 Circuit Relay v2 的 reservation/TTL 机制（见第 5 节） |

来源：[docs.rs Swarm Config](https://docs.rs/libp2p/latest/libp2p/swarm/struct.Config.html)・
[PR #4967](https://github.com/libp2p/rust-libp2p/pull/4967)・
[keep_alive handler](https://docs.rs/libp2p/latest/libp2p/swarm/keep_alive/index.html)・
[js ConnectionManagerInit](https://libp2p.github.io/js-libp2p/interfaces/libp2p.index.ConnectionManagerInit.html)・
[BackoffConnector](https://github.com/libp2p/go-libp2p/blob/master/p2p/discovery/backoff/backoffconnector.go)・
[basic_host.go](https://github.com/libp2p/go-libp2p/blob/master/p2p/host/basic/basic_host.go)

## 2. Tailscale

DERP（Designated Encrypted Relay for Packets）以 curve25519 公钥为地址，仅转发
加密 WireGuard 包，官方定位是"last resort"——直连路径找不到或打不开时才用，并作为
fallback/bootstrap 路径常驻。直连/中继切换由 magicsock 驱动：endpoint 以
heartbeatInterval 为周期对当前最优 UDP 路径发 disco 心跳，同时探测其他路径与 UDP
中继路径；periodicReSTUN 在 20-26s 随机间隔重新反射公网映射；UDP 路径寿命按
10s/30s/60s 三档"cliff"（NAT 超时台阶）主动探测。对端在线状态双通道判定：DERP
服务器通过 FramePeerPresent/FramePeerGone 向区域内订阅者广播客户端上线/下线
（FrameWatchConns 做区域 mesh 同步）；客户端侧按约 10 秒粒度记录对端最后收包时间。
DERP 链路健康：服务器每 60s 发 KeepAlive 帧（含抖动，2 倍间隔无帧即视为断），
另有 FramePing/FramePong 即时校验。

| 维度 | 做法 |
|---|---|
| 保活方式 | disco 心跳维持最优路径；DERP 服务器下发 KeepAlive 帧 |
| 探活间隔 | heartbeatInterval 周期 disco ping；reSTUN 20-26s；DERP 帧 60s |
| 离线判定 | 中继层 presence 事件（PeerGone）+ 客户端 last-recv（约 10s 粒度） |
| 重连策略 | 无显式重连概念：路径劣化自动回落 DERP，心跳探到更优路径自动切回 |
| 中继健康检测 | 60s KeepAlive 帧，2 倍间隔判断；FramePing/Pong 主动校验 |

来源：[derp.go 协议注释](https://github.com/tailscale/tailscale/blob/main/derp/derp.go)・
[endpoint.go](https://github.com/tailscale/tailscale/blob/main/wgengine/magicsock/endpoint.go)・
[magicsock.go](https://github.com/tailscale/tailscale/blob/main/wgengine/magicsock/magicsock.go)

## 3. WebRTC/ICE

状态机：iceConnectionState 从 new 经 checking、connected 到 completed；
end-of-candidates 时若无可用候选对则进入 failed，disconnected 是可恢复的瞬态。
STUN 保活：ICE 的 keepalive 使用 STUN（RFC 8445 第 11 节）；对非 ICE 的通用 UDP
应用，RFC 6263 建议 keepalive 周期 Tr 最小推荐 15s。WebRTC 在其上叠加 consent
freshness（RFC 7675）：默认每 5s 发一个 STUN binding request（各区间随机化 0.8-1.2
倍，实际 4-6s，不得低于 4s），30 秒内未收到有效 binding response 即 MUST 停止在该
5-tuple 上发数据——即离线判定是 30s 无响应；请求/响应同时兼任 NAT 保活与对端同意
校验。链路恢复：ICE restart 可由任一端随时触发（RFC 8445 第 9 节，重发候选信息），
restartIce() 重新走 gather/check：completed 重启后转 connected、disconnected 转 checking；
MDN 指出的常规实践正是监听 state 变为 failed 后触发 ICE restart。

| 维度 | 做法 |
|---|---|
| 保活方式 | STUN binding request/response（consent 检查兼任 NAT keepalive） |
| 探活间隔 | 默认 5s，随机化后 4-6s；通用 UDP 场景下限建议 15s |
| 离线判定 | 30s 无有效响应即停发（consent 过期），状态机进入 failed |
| 重连策略 | 状态机驱动：failed 后由应用触发 ICE restart，重新收集候选 |
| 中继健康检测 | TURN allocation 靠刷新续租； consent 超时同样适用于中继 5-tuple |

来源：[RFC 8445](https://www.rfc-editor.org/rfc/rfc8445.txt)・
[RFC 7675](https://www.rfc-editor.org/rfc/rfc7675.txt)・
[RFC 6263](https://www.rfc-editor.org/rfc/rfc6263.txt)・
[MDN iceconnectionstatechange](https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection/iceconnectionstatechange_event)・
[MDN restartIce()](https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection/restartIce)

## 4. Tox

传输分层：UDP DHT 为主，TCP relay + onion 兜底；好友间通过 SHARE_RELAYS 包互赠
各自可用的 TCP 中继列表（每 120s 一次）。探活分三级：DHT 节点每 60s ping 一次，
122s 无响应即从节点列表除名（响应有效才入列）；好友连接（friend_connection）每 8s
发 ping/Alive 包（FRIEND_PING_INTERVAL=8），32s（间隔×4）收不到任何包即 kill 连接、
判离线；TCP relay 链路上客户端每 30s 发 ping、10s 超时（服务端对称：每 30s ping、
10s 无响应踢除）。打洞前先经 DHT 中继节点发 NAT ping 询问好友"是否在线且在找我"，
收到同随机数响应才启动 hole-punching——在线判定直接服务建连决策。

| 维度 | 做法 |
|---|---|
| 保活方式 | 三级 ping：DHT 节点 ping、好友 ping/Alive 包、TCP relay ping/pong |
| 探活间隔 | DHT 60s；好友 8s；TCP relay 30s |
| 离线判定 | DHT 122s 无响应除名；好友 32s 无包断连；relay 10s pong 超时换中继 |
| 重连策略 | 无退避参数化：好友常驻 DHT 查找列表，掉线后靠 DHT 重新发现 + 中继列表互赠 |
| 中继健康检测 | 30s/10s 的 ping-pong 超时即切换备用中继 |

来源：[tox-rs 协议规格](https://github.com/tox-rs/tox-spec/blob/master/spec.md)・
[friend_connection.h](https://github.com/TokTok/c-toxcore/blob/master/toxcore/friend_connection.h)・
[toxcore tcp client](https://docs.rs/tox/0.0.4/tox/toxcore/tcp/client/index.html)

## 5. IPFS Circuit Relay v2

v2 的核心是"有限中继 + 资源预约"：客户端向中继发 RESERVE 换取带过期时间的
reservation 凭证（expire 为 UTC UNIX 秒），到期前由客户端负责刷新；reservation
仅在到中继的连接存活期间有效，连接断开即失效，中继在 reservations 全部过期后可按
自身连接管理策略回收连接。中继电路本身是受限的：Limit{duration, data}，为 0 表示
不限；超限或超时中继直接 reset 双向流。go-libp2p 默认值：reservation TTL 1 小时、
电路时长 2 分钟、每向数据 128KB、总预约槽 128、每 peer 电路 16、每 IP 预约 8、
每 ASN 预约 32。设计动机明确：限额逼迫打洞——中继同时为 DCUtR 式打洞提供信令
协调，直连升级成功后中继电路即可丢弃。资源类错误码：NO_RESERVATION、
RESOURCE_LIMIT_EXCEEDED、PERMISSION_DENIED、CONNECTION_FAILED。

| 维度 | 做法 |
|---|---|
| 保活方式 | 无独立 keepalive：底层连接存活 + 到期前刷新 reservation |
| 探活间隔 | TTL 驱动（go 默认 1h），刷新周期由客户端按 TTL 自行安排 |
| 离线判定 | 到中继的连接断开 → reservation 立即失效；TTL 到期回收 |
| 重连策略 | 重新 RESERVE 即可；与打洞协调（DCUtR 类）配合促直连升级 |
| 中继健康检测 | 限额超限 reset 流 + 错误码显式回传（NO_RESERVATION 等） |

来源：[circuit-v2 规格](https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md)・
[go-libp2p resources.go](https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go)

## 6. 加分项：WireGuard persistent keepalive 与 BitTorrent DHT

WireGuard：PersistentKeepalive 可配 1-65535 秒，周期发送认证空包维持 NAT/防火墙
映射；手册示例即 25 秒（低于常见 NAT UDP 超时）；端点地址随最近一个正确认证包的
源地址自动更新（漫游即重连）。默认关闭，按需开启。
来源：[wg(8) 手册](https://git.zx2c4.com/wireguard-tools/plain/src/man/wg.8)

BitTorrent DHT（BEP 5）：节点三态——15 分钟内有响应为 good；15 分钟无活动降为
questionable；多次无响应（再试一次后仍失败）判 bad 弃用。桶内 15 分钟未变化即触发
刷新（对桶内范围随机 ID 做 find_node），并优先 ping 最久未见的 questionable 节点。
来源：[BEP 5](https://www.bittorrent.org/beps/bep_0005.html)

## 7. 横向对比

共性做法：
1. 探活都做在应用层、与业务流量解耦（DERP 帧、STUN consent、Tox ping、WG 空包、
   libp2p keep_alive），不依赖传输层自身的存活判定。
2. 离线判定统一是"固定窗口内无响应"，窗口通常为探活间隔的 2-4 倍：DERP 60s→120s、
   ICE 5s→30s、Tox 好友 8s→32s、Tox relay 30s→10s pong 超时、Tox DHT 60s→122s。
3. 中继是兜底而非常态，且主动促成"中继→直连"升级：Circuit v2 用 2min/128KB 限额
   逼打洞；Tailscale 心跳持续探测更优直连；DERP 自述 last resort。
4. TTL 租约统一管理"注册/预约"类状态（rendezvous 注册、circuit v2 reservation、
   BEP 5 token 10 分钟有效），周期刷新动作兼任 NAT 保活。

分歧点：
1. 探活方向：ICE 双向请求-响应、Tox 双向 ping；DERP 由服务端单向下发；WireGuard
   纯单向发送。方向选择决定了能否区分"对端死"与"路径死"。
2. 判死后的行为：WebRTC 30s 停发并进入 failed 等待 restart；libp2p 关连接交上层
   重连；Tailscale 不断链，仅标记路径劣化并常驻中继兜底。
3. 重连策略显式化程度：libp2p 把退避做成可配置组件（factor/interval/retries、
   退避缓存）；WebRTC 交给状态机 + 应用决策；Tox 无参数化，靠 DHT 重新发现。
4. 中继会话寿命语义：circuit v2 显式时限/量限（2min/128KB）；DERP 允许长期驻留；
   ICE 靠 TURN allocation 周期续租。

## 8. 落地建议（对照本仓库架构）

本仓库降级链为直连→打洞→加密中继（docs/design/p2p-base-design.md §7.3/§8），
连接编排契约集中在 crates/p2p-swarm，中继在 crates/p2p-relay，发现在
crates/p2p-discovery。可采纳清单（按优先级）：

| # | 建议采纳项 | 对应 crate 与路径参考 | 优先级 | 预期收益 |
|---|---|---|---|---|
| 1 | 对常驻对端（bootstrap/relay）引入显式重连退避组件：base × factor 指数、次数上限、成功后复位，事件化报告退避状态 | p2p-swarm（backoff.rs、pool.rs） | 高 | 对齐 libp2p 一等退避；消除 E4 轮登记的 relay_session 退避复位语义缺口（coordination.md E5 候选） |
| 2 | 中继电路引入 reservation 式 TTL+刷新：TTL 到期前由客户端刷新，连接断开即失效，TTL 到期回收；限额超限回显式错误码 | p2p-relay（lifecycle.rs、slots.rs、limits.rs、client.rs） | 高 | 对齐 Circuit Relay v2；防配额自锁类缺陷复发（E4 轮 32 槽自锁先例），回收路径可观测 |
| 3 | 中继控制流加 keepalive 帧 + "2 倍间隔判死"，替代依赖 QUIC idle timeout 的粗粒度隐式判定 | p2p-relay（control.rs、client.rs、service.rs） | 中 | 对齐 DERP 60s 帧模式；半开控制流秒级发现，故障归因不再依赖 transport 层 |
| 4 | 空闲连接超时 + "使用中"豁免 + 关闭原因事件化（idle/error/refused 分档） | p2p-swarm（pool.rs、swarm/） | 中 | 对齐 libp2p idle 10s 模型；死连接及时回收且 DialHop 归因更准 |
| 5 | 统一 PeerLiveness 判死语义：rendezvous TTL、mDNS TTL、relay 槽 TTL、QUIC idle 各自阈值汇总为单一活跃度事件源 | p2p-swarm（metrics.rs、swarm/）+ p2p-discovery（cache.rs） | 中 | 消除判定阈值分散；上层业务获得一致的在线/离线事件，避免 M2 轮 mDNS 误报类缺陷 |
| 6 | 路径寿命 cliff 探测（10/30/60s 台阶）验证 NAT 映射剩余寿命，为"中继→直连"升级与保活间隔自适应提供数据 | p2p-swarm（swarm/）+ p2p-relay（punch.rs） | 低 | 对齐 Tailscale probeUDPLifetime；保活间隔可按实测 cliff 自适应而非固定值 |
| 7 | 在线状态呈现采用 last_recv 时间戳（秒级粒度）而非布尔在线 | p2p-swarm（metrics.rs） | 低 | 对齐 Tailscale last-recv 模式；排障时区分"刚失联"与"久未通" |
| 8 | 已对齐项确认：mDNS TTL 续期+过期扫描（"TTL 内无刷新即离线"）、rendezvous 签名注册 TTL、退避健康复位 | p2p-discovery（mdns.rs、rendezvous/client.rs） | 无需动作 | 与 BEP 5/Tox 的窗口判离线语义一致，保持现状并纳入回归 |

优先级依据：1/2 直接对应已登记的 E5 候选与 E4 实证缺陷；3/4 是稳定性轮的低成本
高确定性项；5-7 依赖观测数据，建议先埋指标再调参。

## 参考来源汇总

- libp2p：[Swarm Config](https://docs.rs/libp2p/latest/libp2p/swarm/struct.Config.html)・
  [PR #4967](https://github.com/libp2p/rust-libp2p/pull/4967)・
  [js ConnectionManagerInit](https://libp2p.github.io/js-libp2p/interfaces/libp2p.index.ConnectionManagerInit.html)・
  [BackoffConnector](https://github.com/libp2p/go-libp2p/blob/master/p2p/discovery/backoff/backoffconnector.go)
- Tailscale：[derp.go](https://github.com/tailscale/tailscale/blob/main/derp/derp.go)・
  [endpoint.go](https://github.com/tailscale/tailscale/blob/main/wgengine/magicsock/endpoint.go)・
  [magicsock.go](https://github.com/tailscale/tailscale/blob/main/wgengine/magicsock/magicsock.go)
- WebRTC/ICE：[RFC 8445](https://www.rfc-editor.org/rfc/rfc8445.txt)・
  [RFC 7675](https://www.rfc-editor.org/rfc/rfc7675.txt)・
  [RFC 6263](https://www.rfc-editor.org/rfc/rfc6263.txt)・
  [MDN](https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection/iceconnectionstatechange_event)
- Tox：[tox-rs 规格](https://github.com/tox-rs/tox-spec/blob/master/spec.md)・
  [friend_connection.h](https://github.com/TokTok/c-toxcore/blob/master/toxcore/friend_connection.h)
- IPFS：[circuit-v2](https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md)・
  [resources.go](https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go)
- 加分项：[wg(8)](https://git.zx2c4.com/wireguard-tools/plain/src/man/wg.8)・
  [BEP 5](https://www.bittorrent.org/beps/bep_0005.html)
