# P2P 底座安全审查报告（第 1 期）

- 审查会话：Z（安全审查，只读审计，不改代码）
- 基线：main @ 45616a6（K/P/D/R 四包合并后，S 会话在途改动未纳入）
- 范围：crates/p2p-security、p2p-identity、p2p-discovery、p2p-relay、p2p-protocol 全部源码与测试；
  另核对 p2p-transport（dial 路径）、p2p-mux（流上限）、p2p-swarm / p2p（当前为契约桩）
- 对照声明：docs/design/p2p-base-design.md §5（线协议）/ §6（身份与安全）
- 方法：逐文件通读 + 按六类清单核对；行号以基线 commit 为准

## 结论速览

| 级别 | 数量 | 编号 |
|---|---|---|
| 高 | 1 | H1 |
| 中 | 5 | M1-M5 |
| 低 | 5 | L1-L5 |

最重要的发现是 H1：rendezvous 注册签名未覆盖 TTL，且签名无时间戳等新鲜度材料，
旧注册帧可被永久重放，违背设计 §6「对 (PeerId, 地址, TTL) 签名」的防劫持红线。

## 高危

### H1 rendezvous 注册签名未覆盖 TTL，且无重放窗口

- 位置：crates/p2p-discovery/src/rendezvous/messages.rs:138-146（SignedFields 只有
  namespace/peer_id/addrs）、messages.rs:158-169（sign_register）、messages.rs:190（verify）；
  server.rs:44 直接采信未签名的 reg.ttl_secs
- 问题：签名覆盖的字段集是 {namespace, peer_id, addrs}，TTL 不在其中；签名也不含
  时间戳/序号/nonce。设计 §6 明文要求「对 (PeerId, 地址, TTL) 签名」。
- 攻击场景：
  1. 任何人录到一条合法注册帧（恶意 bootstrap 天然可见；此后不再需要在路径上），
     可把 ttl_secs 改成任意值后重放，verify_register 仍然通过，服务端按篡改后的
     TTL 入库（u32 最大约 136 年）。
  2. 受害者换址/下线后，旧帧持续重放可让旧地址映射无限续命：查询方拿到失效地址
     （拒绝服务），或攻击者配合把流量引向其已控制的旧地址（助劫持）。
- 修复建议：把 ttl_secs 加入 SignedFields（新增 tag=4），并在签名载荷中加入注册时刻
  （或单调计数），服务端校验新鲜度（如允许偏差 ±N 秒）；rendezvous TTL 参照 relay
  的 MAX_TTL_SECS（state.rs:19）加上限截断。
- 补充（反向同步后）：main @ 87e8683 新增的 docs/design/wire-protocol.md:162-165 已把
  本缺口按现状写进线协议规范 v1，与 p2p-base-design.md §6 形成文档间冲突；修复时须
  两份文档与线格式（tag 分配）同步改，且规范冻结越久存量注册帧越难兼容。

## 中危

### M1 rendezvous 服务端无资源上限

- 位置：crates/p2p-discovery/src/rendezvous/server.rs:48-50、63（entry().or_default()）；
  server.rs:29-52（register 无速率限制、namespace 长度不限）；server.rs:44（TTL 无上限）
- 问题：register 与 query 都会为任意 namespace 建表；每表 peer 数、namespace 总数、
  单注册 TTL 上限、单连接注册频率均无约束。
- 攻击场景：单客户端用随机 namespace 高频注册/查询即可撑大 bootstrap 内存；一次
  注册 TTL 136 年的条目在签名被修好（H1）后仍会长期占用。
- 修复建议：namespace 白名单或总量/长度上限；每连接注册速率限制；TTL 截断；
  空 namespace 拒绝。

### M2 relay 电路 Connect 无属主校验，CircuitId 顺序可枚举

- 位置：crates/p2p-relay/src/state.rs:53-54（next_circuit 从 1 顺序自增）、
  state.rs:120-144（on_connect 只查存在性与配额，不校验 joiner 身份）；
  circuit.rs:14-29
- 问题：任何已认证 peer 拿到 cid 即可接入他人电路；cid 顺序发放，先 reserve 一次
  探明当前计数即可向上枚举存活电路。
- 攻击场景：攻击者枚举 cid 并抢先 park 在受害者 A 刚预约的电路上；真正的对端 B
  到达后与攻击者配对。电路内层安全握手（expected PeerId）会失败，机密性不破，
  但 B 的建连失败、电路配额被消耗，可稳定破坏中继可用性（griefing/DoS）。
- 修复建议：reserve 时由服务端生成不可预测 cid（CSPRNG u64）；或 reserve 者声明
  允许接入的 PeerId，on_connect 校验；或首条 parked 流绑定期望 joiner。

### M3 握手与首帧路径普遍缺超时（slowloris 可占资源）

- 位置：crates/p2p-security/src/noise.rs:151-176（recv_frame/send_frame 无 deadline，
  对端发 2 字节长度后挂起即永久等待）；crates/p2p-transport/src/tcp.rs:46-66、77-101
  （TCP accept/dial 与 Noise 握手均无超时）；crates/p2p-transport/src/quic.rs:91-106、
  135-143（accept/dial 未设 max_idle_time，connecting.await 无超时包装）；
  crates/p2p-relay/src/service.rs:103-120（首帧 read_msg 无超时）、control.rs:30-45
  （控制循环无 idle 超时）
- 问题：设计 §6 要求「握手超时」，当前任何一层都未落实。
- 攻击场景：攻击者建立大量半开握手/半开控制流，每条只写几个字节即挂起，耗尽
  任务数、文件描述符与每 peer 链路配额（8 条链路 × 1 控制流可被单身份长期占满）。
- 修复建议：握手全程 tokio::time::timeout 包装（建议 5-10s）；quinn TransportConfig
  设 max_idle_time；relay 首帧与控制流设 idle 超时；TCP dial 的 connect 加超时。

### M4 dial 侧 expected 为 Option，主干尚无生产 dial 路径强制校验

- 位置：crates/p2p-transport/src/lib.rs:44-53（expected: Option<PeerId>）；
  crates/p2p-security/src/noise.rs:137-145（ensure_expected 仅 outbound 且受 Option
  控制）；crates/p2p-transport/src/quic.rs:43-58（None 时不比对）。
  当前主干调用 dial 的只有测试（echo.rs:77,114,147 均传 Some），swarm/p2p 为
  契约桩（p2p-swarm/src/lib.rs、p2p/src/lib.rs）
- 问题：握手即认证保证了「对端是谁」是真的，但「对端是不是我要找的人」完全依赖
  调用方传 Some。发现层输入（rendezvous 应答、mDNS TXT）本身可被恶意 bootstrap /
  局域网攻击者投毒（见 L1），一旦未来 swarm 的某条 dial 路径漏传 expected，
  地址投毒即升级为中间人落地。
- 攻击场景：恶意 bootstrap 对「查 A 得到 B 的地址」的应答返回攻击者地址；若调用方
  未比对 expected，握手会成功（攻击者出示自己的合法身份），调用方误以为连上了 A。
- 修复建议：swarm 的「按 PeerId 拨号」API 把 expected 设为必填参数（类型层面删除
  Option）；按地址盲拨路径保留 Option 并记日志；为 transport 增加回归测试锁定该行为。

### M5 relay 每 Peer 配额可被无限新身份稀释，桶表只增不减

- 位置：crates/p2p-relay/src/limits.rs:85-95（PeerBuckets 只建不清）、service.rs:78-90
  （配额全部按 peer 字符串计）、state.rs:49-55（links/circuit_load 无全局总量上限）
- 问题：所有防滥用记账以「身份」为粒度，而新身份零成本（Sybil）：每身份 8 链路、
  32 电路、1 MiB/s 出口带宽，N 个身份即 N 倍；全服总链路/总电路/总桶数无上限。
- 攻击场景：脚本批量生成身份各挂 8 条链路并 park 电路，占满 relay 连接数与内存；
  PeerBuckets 长期运行内存缓慢增长（每个历史 peer 永久保留一个桶）。
- 修复建议：增加全局总量上限（总链路/总电路/总桶）；PeerBuckets 在链路清零时回收
  桶；登记「信誉」为后续正名（设计 §6 已预留）。

## 低危

### L1 rendezvous 查询应答无签名，客户端全盘采信

- 位置：crates/p2p-discovery/src/rendezvous/client.rs:68-94（response_to_peers 后直接
  入缓存并发 Discovered 事件）；server.rs:55-74
- 问题：应答消息无任何认证材料，只能依赖「与 bootstrap 之间的加密信道」；恶意或
  被入侵的 bootstrap 可投毒地址缓存。设计接受对 bootstrap 的信任，但值得显式记录：
  本条与 M4 构成串联防线，M4 缺失时投毒即落地。
- 修复建议：维持信任模型即可；若要加固，bootstrap 对应答附服务端签名或要求注册者
  出具可离线验证的注册凭证（签名注册帧本身就是，查询方可自行验签）。

### L2 地址字段解析静默降级：端口截断与畸形地址静默丢弃

- 位置：crates/p2p-discovery/src/rendezvous/messages.rs:37-44（port u32 as u16 截断）、
  messages.rs:189-190 与 server.rs:43（filter_map 丢弃解析失败的地址后按过滤集验签）
- 问题：注册里混入畸形地址时，验签集合与存储集合可能不同于签名者的原始集合且无
  任何信号；端口大于 65535 静默截断。均不可被利用来改写有效地址（签名仍约束
  有效地址集），但违反「失败路径留观测信号」的项目红线。
- 修复建议：to_addr 对 port > u16::MAX 返回 None 时，verify_register 直接判 false；
  或在解析失败时显式拒绝整个注册。

### L3 网络路径上的 expect/锁中毒 panic 面

- 位置：crates/p2p-discovery/src/rendezvous/server.rs:47、62（expect("registry lock")）、
  cache.rs:32、57、69（expect("cache lock")）；crates/p2p-relay/src/service.rs:45
  （expect(POISONED)，有意为之但同样 panic）；crates/p2p-security/src/noise.rs:90
  （expect，输入为本端密钥，构造上安全）；crates/p2p-discovery/src/mdns.rs:106
  （expect，静态配置输入）
- 问题：任何一处 Mutex 中毒后，后续网络任务 panic；服务端单 panic 即终止 rendezvous
  或 relay 服务循环（无 catch_unwind）。
- 修复建议：锁中毒改用 PoisonError 恢复或显式错误上抛；服务端 accept 循环外层加
  catch_unwind 或监督重启；网络输入路径保持零 expect。

### L4 p2p-protocol varint 第 10 字节高位静默丢弃

- 位置：crates/p2p-protocol/src/lib.rs:188-204（read_varint 允许满 10 字节且不校验
  溢出位）；对比 crates/p2p-relay/src/messages.rs:159-177（shift>=64 即报错，实现正确）
- 问题：编码超过 64 位的 varint 会被回绕解码，随后帧长上限检查兜底，无内存风险，
  但属互操作隐患，且两 crate 行为不一致。
- 修复建议：read_varint 在第 10 字节非零高位时报 InvalidData（对齐 relay 实现）。

### L5 PeerId 公式与设计文档不一致

- 位置：crates/p2p-identity/src/lib.rs:2-4、18-20（PeerId = sha256(原始 32 字节公钥)，
  展示时 base58）；设计 §6 为「base58(sha256(protobuf(公钥)))」
- 问题：实现内部自洽（握手、证书、rendezvous 三处一致），当前无安全后果；但与
  设计文档及未来 libp2p 互操作（multihash/proto 封装）相悖，文档或实现须改一处。
- 修复建议：定稿前二选一：改文档为「sha256(raw pubkey)」，或在冻结线协议前补
  protobuf/multihash 封装（越晚改代价越大）。

## 已核对无误（防假空白）

- 身份推导：QUIC 侧 peer_id_from_cert（tls_cert.rs:66-105）要求身份扩展公钥与 SPKI
  完全一致，TLS1.3 CertificateVerify 由 rustls 以 SPKI 验签，握手签名另经
  tls_verify.rs:19-34 以扩展公钥复验，仅收 ED25519；Noise 侧 verify_payload
  （noise.rs:117-135）以域分隔签名绑定 X25519 静态钥，PeerId 从 payload 公钥推导。
  全链路未发现采信自报字符串的认证路径；relay 的 peer 字符串由 transport 注入
  （link.rs:17-19 契约），仅 MockLink 可注入任意串（真实实现在途，风险记 M4）。
- 帧长度与分配：p2p-protocol read_frame（lib.rs:143-154）与 relay read_msg
  （messages.rs:192-204）均先校验上限再 vec! 分配，无「声明超大长度先分配」漏洞；
  chunked 重组逐段校验 64 MiB 上限（chunked.rs:66-72）；Noise 流帧长 u16 上限 64 KiB
  （noise_stream.rs:86-87）。
- 加密原语：无手工 nonce 管理（snow/rustls 内部计数）；随机源 OsRng（identity
  lib.rs:49-52）；证书/Noise 均强制双向认证；TLS 仅 TLS1.3 + 固定 ALPN（tls.rs:21、47）；
  未启用 0-RTT（quinn 默认关闭，设计 §6 的 0-RTT 限制暂无暴露面）；PeerId 比较为
  公开值等值比较，无恒定时间需求。
- 限流：relay 每 peer 链路/电路/出口带宽配额齐全且有测试覆盖（circuit_bridge.rs:86-135），
  Reserve TTL 有缺省与 1 小时上限截断（state.rs:17-19、108）；mux 每连接 64 流上限
  （p2p-mux lib.rs:36）并在 yamux 配置与 quinn 传输参数双写。
- 帧写侧：所有 write 路径本地先检上限再写（protocol lib.rs:128-141、relay
  messages.rs:180-189），不会发出超限帧。
- 身份种子落盘：0600 创建、加载时收紧宽松权限、长度不符拒绝（identity seed.rs:26-55），
  测试覆盖（tests/seed.rs:39-61）。
- mDNS：TXT 解码对垃圾输入返回 None 不 panic（mdns.rs:63-76）；自通告去重
  （mdns.rs:114-117）。
- 打洞信令：relay 改写 peer_id 为发送方的已认证身份（control.rs:76-101），接收方
  看到的来源不可伪造（信令内容本身未验签，仅影响探测目标，见 M3/M5 的资源面）。

## 审查清单对照

| 清单项 | 结论 |
|---|---|
| 身份从密码学材料推导 / 无自报采信 | 通过（dial 路径 expected 为 Option，见 M4） |
| 签名覆盖字段完整性 | 不通过：TTL 未覆盖，无重放窗口（H1） |
| 限流与资源 | relay 配额存在但可 Sybil 稀释、无全局上限（M5）；rendezvous 无上限（M1）；电路无属主校验（M2） |
| panic 面 / 长度分配 | 帧长先检后分配，无打爆路径；锁中毒 expect 为残余面（L3） |
| 加密原语 | 通过（nonce/随机源/恒定时间/双向认证均无问题） |
| 声明长度与实际读取一致性 | 通过（三个帧实现均一致） |
| 握手超时 | 不通过（M3） |
