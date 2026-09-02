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
| S 编排装配 | feat/swarm-facade | p2p-S-编排装配（session-e42e5393） | p2p-swarm（连接池/拨号器/门禁/事件总线/退避工具）、crates/p2p facade（Node/Builder 装配、mdns+rendezvous 接线） | 已完成：e2c5519+490fffa+9b121c9 已合并（含 M4 要求：按 PeerId 拨号 expected 必填），make check 全绿 | ✓ 验收通过 |
| U 互操作测试 | test/interop | p2p-U-互操作测试（session-eeecbbdc） | 新建 crates/p2p-itest（跨 crate 接缝测试：握手身份、rendezvous client↔server、relay 消息限流、协议栈大帧） | 已完成：dc90a8d+263f374 已合并，make check 全绿 | ✓ 验收通过 |
| V 文档整理 | docs/organize | p2p-V-文档整理（session-ba31b85b） | 新建 README.md、docs/README.md、docs/design/wire-protocol.md（只写 .md，事实须与代码对齐） | 已完成：87e8683 已合并；wire-protocol 中 rendezvous 签名小节由 D 修复轮同步修订 | ✓ 验收通过 |
| X 构建门禁 | chore/ci-gate | p2p-X-构建门禁（session-b2f5b9b3） | 新建 Makefile、scripts/check/*.sh（fmt/clippy/test/300 行红线扫描），可选 .gitea workflow | 已完成：ab4ad3e+5c171ce+909da14+1d0a88c 已合并，make check 已成为标准验收手段 | ✓ 验收通过 |
| Z 安全审查 | docs/security-review | p2p-Z-安全审查（session-75a4acd6） | 只读审计 security/identity/discovery/relay/protocol 已合并实现，产出 docs/notes/security-review-1.md 分级 findings | 已完成：6c3f7e7 已合并（高1中5低5），findings 已转入修复轮派单 | ✓ 验收通过 |
| T 命令行 | feat/p2p-cli | p2p-T-命令行（session-ee17b807） | 新建 crates/p2p-cli（bootstrap/node/ping/discover 子命令）+ scripts/smoke-cli.sh | 已完成：10d8cee 已合并，make check 全绿 + 冒烟实测 PASS（bootstrap 注册、双节点发现、ping rtt≈7ms） | ✓ 验收通过 |

## 并行规则（各会话必读）

1. 只改自己范围列出的 crate；其他 crate 只读。
2. 已冻结契约（trait 签名、类型形状）不得修改，只能新增；确需改动先报协调会话。
3. 跨 crate 依赖一律对脚手架契约编程（trait/mock/duplex），不等待其他会话。
4. 根 Cargo.toml 只允许向 [workspace.dependencies] 追加条目；合并冲突在 feature 侧消化。
5. 完成后按 AGENTS.md 收尾：worktree 内反向同步用 `git rebase main`（本地私有分支，保持 main 线性，禁止 git merge main）→ 回主树核对 `pwd` + `git branch --show-current` → `git merge --ff-only <分支>` → `git worktree remove` → `git branch -d`。严禁 `git add -A` / `commit -a`，只 add 自己范围的文件。仓库暂无 gitea 远端，push 步骤跳过。
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
- 【已裁决 2026-09-02】L5 PeerId 公式：定稿为 base58(sha256(ed25519 公钥原始 32 字节))，以实现为准，设计文档 §6 已同步修订；libp2p 互操作如需再以新版本号演进。

## 修复轮（安全审查驱动，2026-09-02 派单）

| 修复单 | 负责会话 | 分支 | 范围 | 对应 findings |
|---|---|---|---|---|
| discovery 安全修复 | D（session-3cc0a86e） | fix/discovery-security | crates/p2p-discovery + wire-protocol.md 对应小节 | ✓ 已合入：faa77a6（H1 签名重放窗口+M1 资源上限+L2）、4a0cdd4（L3），make check PASS |
| relay 安全修复 | R（session-43062388） | fix/relay-security | crates/p2p-relay | ✓ 已合入：43517ba（M2 电路号 CSPRNG+属主校验）、79b90b7（M5 全局上限+桶回收）、2b3eda6（L3）、754c6fa（relay varint，P 发现叠加），含 itest 适配，make check PASS |
| 传输超时 | K（session-401422e9） | fix/transport-timeouts | crates/p2p-security + crates/p2p-transport | ✓ 已合入：M3（080deb6+76200ac）+ 第二批（296fe3a yamux 空闲唤醒、c28f61c QuicTransport::close），make check PASS |
| varint 溢出 | P（session-5cb6377a） | fix/protocol-varint | crates/p2p-protocol | ✓ L4 已合入（8a2a6c5）。P 另发现 relay read_varint 同类回绕，已叠加进 R 的单子 |
| M4（expected 必填） | S | fix/swarm-dial-expected | swarm/facade 层强制，不动 transport 冻结签名 | ✓ 已合入（a89dcf5，含投毒回归测试） |
| M3 贯通轮 | feat/m3-degradation-chain → feat/addr-observation | p2p-S-编排装配（session-e42e5393） | crates/p2p-swarm（降级链：直连→打洞→中继电路，逐跳事件）+ crates/p2p（design §7.2 地址观测：bootstrap 告知外部地址并注册） | 拆两批：降级链已合入验收（3942e2e，make check+冒烟 PASS）；地址观测进行中（feat/addr-observation，E3 打洞/被拨前置） | 降级链各跳事件可断言 + 直连阻断回落中继测试 |
| L1 / L5 | 无代码改动 | — | L1 维持信任模型记录在案；L5 裁决改设计文档（已完成） | L1、L5 |

## 变更记录

- 2026-09-02 协调会话创建本表；K/P/D/R 四会话启动；S 排队。
- 2026-09-02 检查轮 1：P、D 合并落地，机械验收通过（cargo test --workspace 43 用例全绿 + clippy -D warnings 零告警；冻结契约未被改；本表无会话触碰）。
- 2026-09-02 检查轮 2：R 验收通过（main 全量 74 用例全绿 + relay clippy 零告警，边界仅 p2p-relay，未触碰本表）。R 的 merge bubble 15bfe50 与 D/K 同理保留不改写。在途仅剩 S（feat/swarm-facade）；S 落地后进 M3 贯通轮（直连→打洞→中继降级链接入 swarm 拨号器）。
- 2026-09-02 检查轮 1 续：K 在检查期间完成合并（2a229e1 + echo.rs），main 全量 57 用例全绿 + clippy 零告警，验收通过。
- 事故记录：K 收尾时在主树用 git add -A 扫走了协调会话未提交的 coordination.md 修改，混入其 chore(skill) 提交（1971e69）。内容无误已确认，但违反"一提交一变更"与并发红线。整改：主树在协调者手里保持 clean；会话收尾严禁 add -A，只 add 自己的文件。
- 流程备注：D 反向同步用 merge 把 merge bubble（f58b869）带进了 main 历史；K 同样产生了 merge commit（67ac369）。已提醒 K/R 改用 `git rebase main` 保持 main 线性。
- 2026-09-02 扩编：启动 U 互操作测试 / V 文档整理 / X 构建门禁 / Z 安全审查 四个不依赖 S 的并行包；规则 5 更新为 rebase 反向同步 + 禁 add -A。
- 2026-09-02 协调者自查：在 bash 里误对未提交的 coordination.md 跑 sed+checkout 丢过一次编辑，已重录。整改：协调表修改只用编辑工具，编辑+提交同轮完成，禁止在 bash 里对它执行任何写命令。
- 2026-09-02 检查轮 3：U/V/X/Z 四包全部合并落地，make check 门禁全绿（X 的门禁本身一并验证）。Z 产出安全审查报告（docs/notes/security-review-1.md：高1中5低5），据此开启修复轮：D/R/K/P 各领修复单，S 收到 expected 必填的接口要求。L5 同轮裁决（改设计文档）。
- 2026-09-02 检查轮 4：S 验收通过（e2c5519+490fffa，make check 全绿，worktree/分支已清理）；P 的 L4 修复（8a2a6c5）已合入。T 命令行会话启动。E2 准备：138 上 cargo 1.97.1 实际已就位（非交互 PATH 未含 ~/.cargo/bin 导致此前误判），已配 rsproxy 镜像、sudo 免密可用、systemd unit 骨架已放 ~/p2p-lab/（端口占位待 T 交付后定稿），ufw 现仅开 22/tcp 部署时需加 QUIC/TCP 端口。修复轮在途：D（H1 高危，尚未见分支）、K（k2）、R（r2）。
- 2026-09-02 检查轮 5：修复轮过半——K 的 M3（080deb6+76200ac）、P 的 L4（8a2a6c5）、S 的 M4（a89dcf5）均已合入 main。在途：D（H1，d2 已开工）、R（r2，叠加 relay varint 回绕）、K 第二批（yamux 空闲开流悬挂 + QUIC close）。S 转待命，M3 贯通轮预定仍派 S。
- 2026-09-02 检查轮 6：R 修复轮验收通过（M2/M5/L3/varint 四件，make check PASS，含 itest 适配）。修复轮余量：D（H1 高危）、K 第二批（yamux/QUIC close）。E2 部署脚本与 E1/E3 runbook 已入库；源码已预同步 15/114/102。
- 2026-09-02 检查轮 7：修复轮全部关闭——D（faa77a6+4a0cdd4，H1/M1/L2/L3）、K 第二批（296fe3a+c28f61c）合入，make check PASS。仅剩 T 命令行在途；T 落地即执行 E2 部署（scripts/deploy-bootstrap-138.sh）与 M3 贯通轮派单。
- 2026-09-02 检查轮 8：T 验收通过（10d8cee，make check 全绿 + 冒烟实测 PASS：bootstrap 注册/双节点发现/ping rtt≈7ms）。E2 部署在 138 上执行中。红线登记（源自 R 审查发现，已通知 S 于 M3 执行）：RelayLink 夹具的 peer_id 必须标注为对端身份，标注成 relay 自身会让属主/配额校验形同虚设。
- 2026-09-02 检查轮 8 续：**E2 完成**——bootstrap 已在 138 systemd 常驻（active，QUIC 3400/udp + TCP 3401/tcp，远端编译 2m48s），跨公网验证通过：15（LAN）经 138 discover 成功查到本机注册节点。实测暴露地址观测缺口（节点只注册 127.0.0.1 监听地址），已作为必做项补进 S 的 M3 任务（design §7.2）。M3 贯通轮已派 S（feat/m3-degradation-chain）。E1/E3 待 M3 落地后按 runbook 执行。
- 2026-09-02 检查轮 9：M3 降级链合入验收（3942e2e，make check + 冒烟复跑 PASS，rtt 6.4ms）；S 正在第二批 feat/addr-observation（地址观测，E3 前置）。
