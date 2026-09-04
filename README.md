# p2p-base

纯通信 P2P 底座：只负责节点身份、连接、传输加密、节点发现、穿透与中继、按协议 ID 的流分发；
消息语义、存储、业务鉴权一律不在底座内。业务通过实现 `ProtocolHandler` 并注册协议 ID 自扩展，
底座自身的发现/中继能力也走同一套 handler 机制，无特权差别（design §2/§9）。

技术栈：Rust + tokio。QUIC（quinn，TLS1.3 证书内嵌公钥）优先，TCP（Noise XX + yamux）兜底；
局域网 mDNS 自动发现，跨网 rendezvous 引导注册/查询，连接降级链为直连 -> 打洞 -> 加密中继。

## 当前进度（2026-09-04）

- 通信内核（M1）达成并经 E 系列观测/修复轮收口：K 传输、P 协议分发、D 节点发现、
  R 中继穿透、S 编排装配全部合并，facade Node 完整可用
  （QUIC 优先、TCP 兜底、mDNS + rendezvous、直连/打洞/中继降级链）。
- 负载感知选路全链落地：中继水位广播 → 客户端 RTT EMA → 满载降级派发。
- IM 聊天全链落地（crates/p2p-chat + GUI + Tauri 接线 + chat_e2e 全链 itest）：
  好友簿、文本/emoji、四类附件、历史分页、发送状态、离线投递；回复引用走契约加法
  （replyTo，契约与后端已合入，GUI 交互收尾在途）。
- 远程支持 P0b 收官：repair-bridge/helper/enforce/playbook 四工件 + 工单全链贯通，
  真机 3 例为人工里程碑（docs/ops/repair-p0b-drill.md）。
- CLI 对等波推进（apps/cli，p2pctl）：CL1-CL3 已合入，CL4 对等守卫+文档在途。
- E10 闲置 LLM 额度共享 Phase 0 在途（llm-share-ledger / llm-share-offer 并行）。
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
| `crates/p2p-log` | 守护进程日志：滚动文件落盘，失败路径告警可观测 |
| `crates/p2p-itest` | 双节点跨 crate 集成测试（发现/中继/聊天全链 E2E） |
| `crates/p2p-cli` | CLI 复用库：echo 协议与节点装配（apps/cli p2pctl 经路径依赖复用） |
| `crates/p2p-chat` | IM 聊天业务层：/im/chat/1 协议、好友簿、消息/附件存储、outbox 离线队列 |
| `crates/repair-bridge` | 远程支持接入桥：runner stdio ⇄ /repair/bridge/1 帧双向对拷 |
| `crates/repair-helper` | MCP 宿主：工具面装配、票据校验、shell_exec 执行与审计 |
| `crates/repair-enforce` | 执法核心（纯逻辑）：红线/scope 门/审批状态机/白名单判定 |
| `crates/repair-playbook` | playbook 解析与 shell_union 白名单数据源 |
| `crates/llm-share-ledger` / `crates/llm-share-offer` | 闲置 LLM 额度共享（E10，在途）：收据账本/能力声明与选路 |

依赖方向：facade -> swarm -> relay/discovery/protocol -> transport/security/mux -> identity；
层间只经 trait 交互，任一层可替换（design §3）。

## 快速上手

```bash
# cargo 装在 ~/.cargo/bin，不在默认 PATH 时先补：
export PATH="$HOME/.cargo/bin:$PATH"

cargo test --workspace                        # 全绿即开发环境就绪
cargo clippy --workspace -- -D warnings       # 提交门禁，零告警
```

全新 clone 后需一次性引导 git 钩子（新建 worktree 自动接入 .env，机制见
docs/ops/env-worktree-bootstrap.md）：

```bash
git config core.hooksPath "$(pwd)/githooks"   # 在仓库根执行，必须绝对路径
```

无需外部服务即可跑通全部单测（mDNS 真实组播用例默认 `#[ignore]`）。
改代码请先读 `AGENTS.md`（分支/worktree/提交纪律）与 `docs/coordination.md`（并行规则）。

## 文档

- 索引：[docs/README.md](docs/README.md)
- 总方案：[docs/design/p2p-base-design.md](docs/design/p2p-base-design.md)
- 线协议：[docs/design/wire-protocol.md](docs/design/wire-protocol.md)
- GUI 契约：[docs/design/gui-contract.md](docs/design/gui-contract.md)
- IM 聊天设计：[docs/design/im-chat-design.md](docs/design/im-chat-design.md)
- 发布门禁：[docs/release-gates.md](docs/release-gates.md)
