# P2P 底座并行开发协调表

维护人：协调会话（主会话）。各开发会话禁止修改本文档，进度由协调者统一更新。
契约来源：docs/design/p2p-base-design.md；脚手架契约（trait 签名/类型形状）已冻结，改动需经协调会话。

## 包与负责人

| 包 | 分支 | 负责会话 | 范围（拥有的 crate） | 状态 | 验收 |
|---|---|---|---|---|---|
| K 内核传输 | feat/kernel-transport | p2p-K-内核传输 | p2p-transport（quinn QUIC + TCP）、p2p-security（TLS1.3 + Noise XX）、p2p-mux（yamux 实现）、p2p-identity（种子落盘持久化） | 已完成：83eaafe/f2bbb03/2c84ae2/2a229e1 已合并，QUIC+TCP 双路 echo 集成测试在 crates/p2p-transport/tests/echo.rs，clippy -D warnings 零告警 | ✓ 验收通过 |
| P 协议分发 | feat/protocol | p2p-P-协议分发 | p2p-protocol（RequestResponse 实现、开流协议握手助手、chunked transfer） | 已完成：84c4337+db91134 已合并，14 用例全绿 + clippy -D warnings 零告警 | ✓ 验收通过 |
| D 节点发现 | feat/discovery | p2p-D-节点发现 | p2p-discovery（mdns-sd 局域网发现、rendezvous 客户端 + 签名注册、AddrCache 实现） | 已完成：f22aebc+7abd318+f1c0105 已合并，27 用例全绿（真实组播 #[ignore]）+ clippy 零告警 | ✓ 验收通过 |
| R 中继穿透 | feat/relay | p2p-R-中继穿透 | p2p-relay（RelayService 服务端、客户端 reserve/connect、打洞信令消息、密文桥接） | 已完成：789a998/4cdbefc/c6d068c/59e7391/0dfdfd0 经 15bfe50 合入，17 用例全绿（256KB 互通/未知电路拒绝/限流断链/prost roundtrip/打洞时序）+ clippy 零告警，最大文件 257 行 | ✓ 验收通过 |
| S 编排装配 | feat/swarm-facade | p2p-S-编排装配（session-e42e5393） | p2p-swarm（连接池/拨号器/门禁/事件总线/退避工具）、crates/p2p facade（Node/Builder 装配、mdns+rendezvous 接线） | 进行中：2026-09-02 启动（K+P 已合并；R 在途，S 不碰 relay） | 两 Node 经 facade 互拨 request roundtrip、事件可见（crates/p2p/tests/facade.rs） |

## 并行规则（各会话必读）

1. 只改自己范围列出的 crate；其他 crate 只读。
2. 已冻结契约（trait 签名、类型形状）不得修改，只能新增；确需改动先报协调会话。
3. 跨 crate 依赖一律对脚手架契约编程（trait/mock/duplex），不等待其他会话。
4. 根 Cargo.toml 只允许向 [workspace.dependencies] 追加条目；合并冲突在 feature 侧消化。
5. 完成后按 AGENTS.md 收尾：worktree 内 `git merge main` 反向同步 → 回主树核对 `pwd` + `git branch --show-current` → `git merge --ff-only <分支>` → `git worktree remove` → `git branch -d`。仓库暂无 gitea 远端，push 步骤跳过。
6. 最终回复报告：分支名、提交列表、测试结果摘要、遗留问题；不自行修改本表。
7. 环境：cargo 在 ~/.cargo/bin，命令前 `export PATH="$HOME/.cargo/bin:$PATH"`。
8. 编码红线：单文件 ≤300 行、函数 ≤60 行、失败路径必须留日志/错误信号、不用 emoji。

## 里程碑映射（design §13）

- M1 通信内核 = K + P 合并后同机两进程互通
- M2 局域网 = S 装配 + mDNS 接线（含断线重连工具）
- M3 跨网 = rendezvous + relay + 降级链贯通（D、R 合并后 S 补接线）
- M4 收尾 = 打洞实测、metrics、p2p-cli、gossip pubsub（可选）

## 待裁决事项

- TransportError 细分：当前仅 Dial/Handshake/PeerMismatch 三类（K 提出）。S 事件总线或 M3 降级链若需更细粒度（超时/拒绝/不可达），属冻结契约变更，须由协调会话裁决：优先"新增枚举变体 + 默认事件文案"的加法路径，禁止改已有变体形状。

## 变更记录

- 2026-09-02 协调会话创建本表；K/P/D/R 四会话启动；S 排队。
- 2026-09-02 检查轮 1：P、D 合并落地，机械验收通过（cargo test --workspace 43 用例全绿 + clippy -D warnings 零告警；冻结契约未被改；本表无会话触碰）。
- 2026-09-02 检查轮 2：R 验收通过（main 全量 74 用例全绿 + relay clippy 零告警，边界仅 p2p-relay，未触碰本表）。R 的 merge bubble 15bfe50 与 D/K 同理保留不改写。在途仅剩 S（feat/swarm-facade）；S 落地后进 M3 贯通轮（直连→打洞→中继降级链接入 swarm 拨号器）。
- 2026-09-02 检查轮 1 续：K 在检查期间完成合并（2a229e1 + echo.rs），main 全量 57 用例全绿 + clippy 零告警，验收通过。
- 事故记录：K 收尾时在主树用 git add -A 扫走了协调会话未提交的 coordination.md 修改，混入其 chore(skill) 提交（1971e69）。内容无误已确认，但违反"一提交一变更"与并发红线。整改：主树在协调者手里保持 clean；会话收尾严禁 add -A，只 add 自己的文件。
- 流程备注：D 反向同步用 merge 把 merge bubble（f58b869）带进了 main 历史；K 同样产生了 merge commit（67ac369）。已提醒 K/R 改用 `git rebase main` 保持 main 线性。
