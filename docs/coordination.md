# P2P 底座并行开发协调表

维护人：协调会话（主会话）。各开发会话禁止修改本文档，进度由协调者统一更新。
契约来源：docs/design/p2p-base-design.md；脚手架契约（trait 签名/类型形状）已冻结，改动需经协调会话。

## 包与负责人

| 包 | 分支 | 负责会话 | 范围（拥有的 crate） | 状态 | 验收 |
|---|---|---|---|---|---|
| K 内核传输 | feat/kernel-transport | p2p-K-内核传输 | p2p-transport（quinn QUIC + TCP）、p2p-security（TLS1.3 + Noise XX）、p2p-mux（yamux 实现）、p2p-identity（种子落盘持久化） | 进行中 | cargo test -p p2p-transport -p p2p-security -p p2p-identity 全绿；同机 QUIC 互拨握手 + mux 流 echo 集成测试 |
| P 协议分发 | feat/protocol | p2p-P-协议分发 | p2p-protocol（RequestResponse 实现、开流协议握手助手、chunked transfer） | 已完成：84c4337+db91134 已合并，14 用例全绿 + clippy -D warnings 零告警 | ✓ 验收通过 |
| D 节点发现 | feat/discovery | p2p-D-节点发现 | p2p-discovery（mdns-sd 局域网发现、rendezvous 客户端 + 签名注册、AddrCache 实现） | 已完成：f22aebc+7abd318+f1c0105 已合并，27 用例全绿（真实组播 #[ignore]）+ clippy 零告警 | ✓ 验收通过 |
| R 中继穿透 | feat/relay | p2p-R-中继穿透 | p2p-relay（RelayService 服务端、客户端 reserve/connect、打洞信令消息、密文桥接） | 进行中 | cargo test -p p2p-relay 全绿；进程内 relay + 两客户端电路字节互通集成测试 |
| S 编排装配 | （待开分支） | （待分配，排队中） | p2p-swarm（连接池/拨号器/门禁/事件总线）、crates/p2p facade（Node/Builder 装配、mdns+rendezvous 接线） | 排队中：待 K 合并后启动（P 已就绪） | 两进程经 facade 互拨、业务 handler 收发、断线事件可见 |

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

## 变更记录

- 2026-09-02 协调会话创建本表；K/P/D/R 四会话启动；S 排队。
- 2026-09-02 检查轮 1：P、D 合并落地，机械验收通过（cargo test --workspace 43 用例全绿 + clippy -D warnings 零告警；冻结契约未被改；本表无会话触碰）。K 进行中（security/mux/identity 已提交），R 未提交。
- 流程备注：D 反向同步用 merge 把 merge bubble（f58b869）带进了 main 历史；已提醒 K/R 反向同步改用 `git rebase main` 保持 main 线性。
