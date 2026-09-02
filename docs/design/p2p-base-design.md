# P2P 通信底座设计方案 v0

状态：设计稿（未实现）。本文只定义通信职责，业务语义一律不在底座内。

## 1. 需求输入（已确认的决策）

| 决策点 | 结论 |
|---|---|
| 网络场景 | 局域网节点 mDNS 自动发现；跨局域网通过公共公网节点自动发现 |
| 技术栈 | Rust + tokio 异步运行时 |
| 交付形态 | 嵌入库/SDK，业务进程内直接使用 |
| 中心设施 | 接受轻量引导节点（rendezvous 发现 + relay 中转），不存业务数据 |

## 2. 定位与边界

底座负责（且仅负责）：

- 节点身份：密钥对、PeerId、身份存续
- 连接：拨号/监听、多路复用、保活、重连、背压、限流
- 安全：传输加密、身份认证、连接门禁（allow/deny 钩子）
- 发现：局域网 mDNS、公网 rendezvous、地址观测
- 穿透与中继：UDP/QUIC 打洞，打洞失败走加密中继
- 分发：按协议 ID 把流路由给业务 handler
- 原语：request-response、可选 gossip pubsub（纯通信原语，不含业务语义）

底座不负责：

- 消息内容语义、业务鉴权授权、存储、路由策略、UI
- 业务协议的编码（底座只提供定长帧透传，业务自选 JSON/CBOR/protobuf）

设计原则：底座自身的能力（发现、中继、ping）也通过同一套 handler 注册机制实现，
与业务协议无特权差别——扩展机制被底座自身 dogfooding，保证它够用。

## 3. 总体架构（分层）

```
┌──────────────────────────────────────────────┐
│ 业务扩展层   业务自选编码/语义/状态机            │
├──────────────────────────────────────────────┤
│ 协议分发层   协议ID路由 → handler 注册表        │
├──────────────────────────────────────────────┤
│ 流复用层     一条连接 → 多条独立流（背压隔离）    │
├──────────────────────────────────────────────┤
│ 安全层       Noise XX (TCP) / TLS1.3 (QUIC)   │
├──────────────────────────────────────────────┤
│ 传输层       QUIC 优先，TCP 兜底；地址拨号抽象   │
├──────────────────────────────────────────────┤
│ 发现与穿透   mDNS / rendezvous / relay / 打洞   │
└──────────────────────────────────────────────┘
```

自上而下依赖，层间只经 trait 交互，任一层可替换。

## 4. 核心抽象与 API 表面（业务视角）

```rust
// 1. 构建节点
let node = Node::builder()
    .keypair(load_or_generate("node.key")?)     // 身份持久化，重启不变
    .listen_on("/ip4/0.0.0.0/udp/0/quic-v1")?   // 同时可听 tcp
    .with_mdns()                                 // 局域网发现
    .with_rendezvous(vec![bootstrap_addr])       // 公网发现注册
    .with_relay()                                // 中继兜底
    .max_frame_size(1 << 20)                     // 单帧上限，防滥用
    .build().await?;

// 2. 注册业务协议（底座只做路由）
node.handle_protocol("/myapp/chat/1", ChatHandler);

// 3. 主动开流/收流
let mut s = node.new_stream(peer_id, "/myapp/chat/1").await?;
s.send(frame_bytes).await?;
let reply = s.recv().await?;

// 4. request-response 便捷原语（带超时）
let resp = node.request(peer_id, "/myapp/echo/1", payload, Duration::from_secs(5)).await?;

// 5. 事件订阅：peer discovered/connected/disconnected、地址变化
while let Some(ev) = node.events().next().await { /* 业务自行决策 */ }

// 6. 连接门禁钩子（通信层安全，不是业务鉴权）
node.gate(|peer_id, addr| allowlist.contains(&peer_id));
```

命名空间（rendezvous 用）是底座提供的唯一分组原语，其业务含义由业务层定义。

## 5. 线协议设计

### 5.1 连接建立与升级

```
原始连接(QUIC stream 或 TCP)
  → 安全握手: QUIC=TLS1.3(证书携带身份公钥, 类 libp2p-tls) / TCP=Noise XX
  → 双方互认 PeerId（握手即带身份，无明文阶段）
  → 复用层: QUIC 原生多流; TCP 挂 yamux
  → 每条流开头: varint(len) + 协议ID(UTF-8)  →  路由到 handler
  → 之后: varint(len) + payload（业务透传，底座不解析）
```

### 5.2 帧

- 前缀：varint 长度 + 字节 payload，单帧默认上限 1 MiB（可配）
- 协议 ID 命名：`/<应用名>/<协议名>/<版本>`，如 `/p2p-base/rendezvous/1`
- 版本不兼容 = 流直接关闭并上报事件，不猜测降级

### 5.3 编码

- 控制面（发现/中继/ping/identify）：protobuf（prost），字段可演进
- 数据面：不透明字节，业务自定

### 5.4 内置控制协议（与业务同机制注册）

| 协议 ID | 职责 |
|---|---|
| `/p2p-base/identify/1` | 交换公钥、监听地址、协议版本、观测地址 |
| `/p2p-base/ping/1` | 往返延迟探测、连通性保活 |
| `/p2p-base/rendezvous/1` | 注册/刷新/查询节点地址（带 TTL） |
| `/p2p-base/relay/1` | 中继申请、打洞协调信令 |
| `/p2p-base/circuit/1` | 加密中继数据桥接 |

## 6. 身份与安全

- 身份：Ed25519 密钥对；PeerId = base58(sha256(ed25519 公钥原始 32 字节))（定稿裁决 2026-09-02：
  实现取 raw 公钥哈希，不做 protobuf/multihash 封装，内部全链路自洽；libp2p 互操作如需再以
  新版本号演进）；密钥落盘 config 目录
- 加密：每连接强制加密，无明文模式；QUIC 用内嵌公钥的自签证书，TCP 用 Noise XX
- 握手即认证：PeerId 与密钥绑定，冒充他人在握手即失败
- 门禁：连接级 allowlist/denylist 钩子
- 通信层防滥用（非业务行为治理）：连接数/流数上限、单帧上限、握手超时、
  协议违规即断链并记入信誉（只记传输行为：超速、非法帧、握手失败频率）
- rendezvous 防抢占：注册必须用身份私钥对 (PeerId, 地址, TTL) 签名，防止劫持他人 PeerId 的地址
- 0-RTT 限制：0-RTT 仅允许幂等控制消息（注册/查询），业务流强制 1-RTT，防重放

## 7. 节点发现与穿透

### 7.1 局域网：mDNS

- 周期通告（默认 5s）：服务名自定义（如 `_p2pbase._udp.local`），
  TXT 记录携带 PeerId 与 quic/tcp 端口
- 通告 TTL 内未刷新即判离线，发 disconnected 事件
- 同机多实例、接口变更（Wi-Fi 切换）需去重与重通告

### 7.2 跨网：rendezvous（公网引导节点）

```
节点A(公网可直连) ──注册(签名地址,TTL)──▶ bootstrap 节点表
节点B(私网) ──查询 PeerA──▶ bootstrap 返回 A 的地址列表 ──▶ B 直拨 A
```

- bootstrap 无状态（只有带 TTL 的注册表），不参与业务
- 节点同时上报两类地址：监听地址 + 从对端观测到的映射地址（NAT 公网出口）
- 客户端缓存最近查询结果（last-known-good），bootstrap 不可达时降级用缓存直拨
- 多 bootstrap 地址配置，防单点

### 7.3 穿透与中继（按优先级降级）

1. 直连：地址可达即直连（QUIC 优先）
2. 打洞：经 relay 信令协调（类 DCUtR）——双方互发探测包同时开洞，QUIC 场景成功率较高
3. 中继：打洞失败，经 bootstrap/公网节点建加密电路
   （relay 只桥接两个密文连接的两端字节流，无法解密，见 5.4 circuit）

地址观测顺序：identify（对端告知）→ bootstrap 告知（CONNECT 反馈）→ 打洞验证。

## 8. 传输与可靠性

- QUIC（quinn）：原生流复用、连接迁移（换网不断）、0-RTT 重连、TLS1.3
- TCP 兜底：tokio TcpStream + yamux + Noise（老环境无 UDP 时）
- 背压：每流独立发送窗口；发送队列有界，队满即对上层报错，不无限堆积
- 保活与重连：keepalive 间隔可配；底座不做业务重连决策，
  提供 `node.connect(peer_id)` 幂等拨号 + 断线事件，业务决定重连策略（附指数退避工具）
- 不可靠通道：QUIC datagram 作为可选通道暴露，业务自选（视频/实时场景）

## 9. 扩展机制（业务如何接入）

| 扩展点 | 机制 |
|---|---|
| 新业务协议 | 实现 `ProtocolHandler` trait 并注册协议 ID，收流即回调 |
| 自定义发现 | 实现 `Discovery` trait（mDNS/rendezvous 均是该 trait 的内置实现） |
| 自定义传输 | 实现 `Transport` trait（dial/listen 抽象） |
| 事件 | 统一事件总线（发现/连接/流/错误），业务订阅 |
| 原语库 | request-response、gossip pubsub（可选模块）、chunked transfer |

新增能力一律先做成 trait + 内置默认实现，避免硬编码分支。

## 10. 模块划分（Cargo workspace，单文件不超 300 行）

```
crates/
  identity     密钥对、PeerId、序列化与签名
  transport    Transport trait + quic(quinn)/tcp 实现 + 地址观测
  security     Noise(snow) / TLS 证书构造、握手状态机
  mux          yamux 封装（QUIC 侧直通原生流）
  protocol     帧、协议ID、varint、handler 注册表、request-response
  discovery    Discovery trait + mdns + rendezvous-client + 地址缓存
  relay        relay 服务端 + client/circuit + 打洞协调
  swarm        连接池、拨号器、门禁、事件总线、限流
p2p            facade：Node/Builder/配置，预组装上述 crate
```

## 11. 关键流程

### 11.1 启动

```
加载/生成身份 → 监听(QUIC+TCP) → mDNS 开始通告 →
向 bootstrap 注册(签名,TTL) → identify 收集观测地址 → 周期刷新注册
```

### 11.2 跨网建连（B 找 A）

```
B: 查 rendezvous 得 A 地址 ──▶ 直拨成功? 用之
   ├─ 失败 ──▶ 经 bootstrap 向 A 发打洞请求 ──▶ 双方同时探测 ──▶ 成功? 用之
   └─ 仍失败 ──▶ 建中继电路（A、B 都连 relay，relay 桥接字节）──▶ 在电路上跑安全握手
```

### 11.3 一次业务请求

```
业务: node.request(peerA, "/myapp/echo/1", payload)
底座: 连接池取/建连接 → 开流 → 写协议ID → 写帧 → 等回帧/超时 → 返回
对端: 分发层按协议ID路由 → 业务 handler 收到流 → 读帧处理 → 回帧 → 流关闭
```

## 12. 可观测性

- tracing 结构化日志：握手、拨号、发现、降级路径（直连→打洞→中继）全程打点
- metrics：连接数/流数、握手成功率、打洞成功率、中继带宽、各协议流量
- 失败路径显式信号：所有降级与错误必发事件/日志，禁止静默吞错
- 调试工具：`p2p-cli`（identify/ping/discover 子命令），独立小 crate

## 13. 分期路线

| 里程碑 | 内容 | 验收 |
|---|---|---|
| M1 通信内核 | identity+transport+security+mux+protocol 分发+request-response | 同机两进程互拨，业务 handler 收发帧 |
| M2 局域网 | mDNS 发现+事件+限流+保活 | 两台设备零配置互见互联 |
| M3 跨网 | rendezvous+relay+地址观测+降级链 | 两个不同内网节点经 bootstrap 互联，直连失败自动中继 |
| M4 收尾 | 打洞、metrics、p2p-cli、gossip pubsub（可选） | 打洞成功率有数据；压测连接/流数达标 |

## 14. 风险与开放决策点

| 风险/决策 | 说明 | 对策 |
|---|---|---|
| 自研 vs libp2p | rust-libp2p 功能全但重、定制难 | 自研轻量版，概念对齐 libp2p；trait 隔离保留替换余地 |
| 打洞成功率 | 对称 NAT 场景低 | relay 永远兜底，降级链可观测 |
| bootstrap 单点 | 引导节点宕机无法新发现 | 多地址配置 + last-known-good 缓存 + 已建连接继续可用 |
| rendezvous 劫持 | 恶意注册他人 PeerId | 注册签名验证（见 6） |
| relay 滥用 | 被当免费带宽 | 每 Peer 限速/限连接，超额断链记信誉 |
| 同机多实例测试 | 端口/mDNS 冲突 | 0 端口随机监听 + 实例隔离的 config 目录 |
