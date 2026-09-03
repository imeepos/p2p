# p2p-base

纯通信 P2P 底座：只负责节点身份、连接、传输加密、节点发现、穿透与中继、按协议 ID 的流分发；
消息语义、存储、业务鉴权一律不在底座内。业务通过实现 `ProtocolHandler` 并注册协议 ID 自扩展，
底座自身的发现/中继能力也走同一套 handler 机制，无特权差别（design §2/§9）。

技术栈：Rust + tokio。QUIC（quinn，TLS1.3 证书内嵌公钥）优先，TCP（Noise XX + yamux）兜底；
局域网 mDNS 自动发现，跨网 rendezvous 引导注册/查询，连接降级链为直连 -> 打洞 -> 加密中继。

## 当前进度（2026-09-02）

- 已合并：K 内核传输（transport/security/mux/identity）、P 协议分发（protocol）、
  D 节点发现（discovery）、R 中继穿透（relay）——通信内核（M1）达成。
- 在途：S 编排装配（p2p-swarm 连接池/门禁/事件总线 + crates/p2p facade 接线），
  facade 当前 `build()` 返回 `NotYetAssembled`，装配落地后各 crate 才串成完整 Node。
- 并行中：U 互操作测试、X 构建门禁、V 文档整理、Z 安全审查。
- 分包、分支与验收口径见 `docs/coordination.md`（协调者维护，各会话只读）。

## Crate 地图

| crate | 职责 |
|---|---|
| `crates/p2p-identity` | Ed25519 密钥对、PeerId（base58(sha256(公钥))）、种子落盘（0600 权限） |
| `crates/p2p-transport` | Transport trait + QUIC(quinn)/TCP(tokio) 实现、TransportAddr、SecureConn |
| `crates/p2p-security` | 安全升级：QUIC 用 TLS1.3 自签身份证书，TCP 用 Noise XX；握手即互认 PeerId |
| `crates/p2p-mux` | 流复用抽象：QUIC 原生流 / yamux 统一为 BoxedStream，每连接 64 流上限 |
| `crates/p2p-protocol` | 帧（varint 长度前缀 + 1 MiB 上限）、ProtocolId、handler 注册表、request-response、chunked transfer |
| `crates/p2p-discovery` | mDNS 局域网发现、rendezvous 签名注册/查询客户端、带 TTL 地址缓存 |
| `crates/p2p-relay` | 中继服务端/客户端（reserve/connect）、打洞信令、密文桥接、每 Peer 限流；水位经 Reserved/KeepAliveAck 广播供负载感知选路；内置协议 ID 常量在此登记 |
| `crates/p2p-swarm` | 连接编排契约：NodeEvent 事件、ConnectionGate 门禁（S 阶段填实现）、多中继负载感知降级派发（满载沉底/RTT 决胜/失败换候选） |
| `crates/p2p` | 对外 facade：Node/NodeBuilder/NodeConfig，S 阶段预组装上述全部 crate |

依赖方向：facade -> swarm -> relay/discovery/protocol -> transport/security/mux -> identity；
层间只经 trait 交互，任一层可替换（design §3）。

## 快速上手

```bash
# cargo 装在 ~/.cargo/bin，不在默认 PATH 时先补：
export PATH="$HOME/.cargo/bin:$PATH"

cargo test --workspace                        # 全绿即开发环境就绪
cargo clippy --workspace -- -D warnings       # 提交门禁，零告警
```

无需外部服务即可跑通全部单测（mDNS 真实组播用例默认 `#[ignore]`）。
改代码请先读 `AGENTS.md`（分支/worktree/提交纪律）与 `docs/coordination.md`（并行规则）。

## 文档

- 索引：[docs/README.md](docs/README.md)
- 总方案：[docs/design/p2p-base-design.md](docs/design/p2p-base-design.md)
- 线协议：[docs/design/wire-protocol.md](docs/design/wire-protocol.md)
