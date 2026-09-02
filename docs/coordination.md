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
5. 完成后按 AGENTS.md 收尾：worktree 内反向同步用 `git rebase main`（本地私有分支，保持 main 线性，禁止 git merge main）→ 回主树核对 `pwd` + `git branch --show-current` → `git merge --ff-only <分支>` → `git worktree remove` → `git branch -d`。严禁 `git add -A` / `commit -a`，只 add 自己范围的文件。远端为 origin（github），收尾时 `git push origin <分支>`（AGENTS.md 中 gitea 名称不适用本仓库）。
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

## E4 调优轮（2026-09-02 派单，目标：E1/E3 复测三连稳定通过）

| 修复单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| 拨号可观测性+refused/hairpin 边角 | S（session-e42e5393） | fix/e4-dialhop-observability ✓ 已合入（00a775f，任务3拆出） | crates/p2p-swarm：DialHop 逐跳归因从 debug 提升为 info 或事件化（采样不开 RUST_LOG=debug 也能归因）；复核检查轮14立案——TCP 入站 refused 在直连跳不得作为最终错误上抛，须继续尝试其余地址；同 NAT hairpin refused 快速失败不占满拨号预算 | make check 全绿 + itest 断言各跳事件 |
| 发现时序+日志降噪 | p2p-D-E4（session-cc9c45f8） | fix/e4-discovery-stability ✓ 已合入（538bebb） | crates/p2p-discovery：发现窗口时序（启动初期错过 mDNS 公告的补救）；rendezvous 盲拨周期 WARN 降 debug 或加退避，仅首次失败保留 WARN | make check 全绿 + 35s 周期刷屏消除的单测断言 |
| ECS 公网节点部署 | p2p-T-ECS（session-814cafa1） | feat/ecs-deploy ✓ 已合入（011c35e+2b8fc6d） | scripts/deploy-bootstrap-ecs.sh（systemd 常驻，QUIC 3400/udp + TCP 3401/tcp + 观测 3402/udp + relay 3403/udp、3404/tcp 已由协调者放行）+ runbook 双公网拓扑条目；部署/观测反射/discover 冒烟 PASS，ping 未通定性为产品缺口（CLI 未接 relay） | make check 全绿 + 15↔ECS 冒烟 PASS ✓（PeerId 入册 §8.5） |
| hairpin 快速失败 | p2p-S-E4（session-7a31fb74） | fix/e4-hairpin-fastfail ✓ 已合入（0f1c73b） | crates/p2p-swarm + itest：验证优先——先以 itest 复现检查轮16 场景，实测达标则只交测试不改逻辑（S 报告 refused 已走快速失败路径） | make check 全绿 + itest 断言 hairpin 场景预算 |
| CLI relay/bootstrap 接线 | p2p-T-ECS（session-814cafa1） | feat/cli-relay-wiring ✓ 已合入（363b2a5） | node 暴露 --relay、--bootstrap 多值、ping DialHop 打印；双 bootstrap discover PASS，中继兜底 ping 被存量 relay 缺陷阻断（定性准确） | make check 全绿 + 双 bootstrap 冒烟 PASS ✓ |
| relay 兜底修复（关键路径） | p2p-R-E4（session-e5aeee3e） | fix/e4-relay-resilience | 根因双实锤：a) quic.rs IdleTimeout 单位错——30s 误传 as_secs，实际 30ms，全系统 QUIC 静默即死（rendezvous"5s 常态"实为症状，检查轮24 定性据此更正）；b) 控制流关闭不释放未配对电路，32 槽自锁（loopback 100% 复现）。裁决：①授权 R 代修 transport 两处毫秒换行（带 >3s 静默存活回归，K 知情）；②b 修复批准（注册载体电路随控制流存亡）；③backoff 复位语义维持现状，登记 E4 余量 | make check 全绿 + itest 转回归 + 15↔ECS 中继兜底 ping PASS |
| TCP 会话即断（/t3401 握手后断） | p2p-K-E4（session-bf2bc941）→ 修复派 S（session-e42e5393） | fix/e4-tcp-stream | 诊断完成：facade TransportLink::connect 丢弃 SecureConn 触发 YamuxMux close-on-drop 自毁（消融实验实证，QUIC 因驱动任务持有连接不受影响）；裁决采纳方案 A（SecureConn 挂进 stream_to_conn 写任务闭包），S 就该分支实现，K 的回归 4000845 即机械验收 | make check 全绿 + 4000845 回归绿 |
| 长稳采样·脚本准备 | p2p-T-ECS（session-814cafa1） | feat/e4-sampling-scripts | scripts/ + docs/ops：默认日志 p2p_swarm=info、禁 pkill（精确 PID）、三连采样结构化统计、双 bootstrap/双 relay 参数化 | make check 全绿 + 脚本自检可运行 |
| 长稳采样·执行 | 待派（依赖 relay 修复 + 脚本就绪） | — | 按 §8.5 runbook 执行 15/102/138/ECS 采样 | E1/E3 复测三连稳定 |

metrics（M4 余项）与 gossip pubsub（可选）排 E5，不在本轮。

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
- 2026-09-02 检查轮 11：S 地址观测合入验收——e492a3e（UDP 反射学公网映射地址并注册进 rendezvous），实测：15 经 138 discover 到 coordinator 的地址列表含 240e: 全局 IPv6（不再只有 127.0.0.1）。15/102 已更新二进制并带 --bootstrap 重启。在途：D 的 TTL 刷新（fix/mdns-ttl-refresh，文件活动中）。
- 2026-09-02 检查轮 12（E3 首证）：102→coordinator（跨公网，经 138 rendezvous 发现）ping 成功，rtt=3.06s——降级链在真实互联网首次闭环。待办：a) DialHop 归因需 RUST_LOG=debug（info 不可见，E3 采样脚本要带）；b) rendezvous 盲拨 WARN 每 35s 刷屏（L1 预期内，噪音可降级 debug）；c) D 的 TTL 刷新修复单在途（d4 有文件活动）；d) 拓扑澄清：本工作机即 192.168.0.15（maca），coordinator 与 maca 同机，E1/E3 结果解读需注意。
- 2026-09-02 检查轮 13：S 报告地址观测实现（UDP 反射协议，声明冻结契约偏差：SecureConn/Response 形状不动，改走独立反射器；cone NAT 端口保持假设，对称 NAT 由中继兜底）——机制认可。发现 138 部署的 bootstrap 为旧版本且 p2p-cli 未接线观测功能：已派 T 追加单（bootstrap --observation-port / node --observation），138 需重部署（部署脚本待 T 交付后同步更新 --observation-port 与 ufw 3402/udp 已预开）。
- 2026-09-02 检查轮 14（E3 采样首批）：102→coordinator 三连 ping：11.9ms（直连 v6）/ 3.04s（慢路径）/ 1 次 TCP Connection refused 失败。发现两个采样期问题：a) 混合成功/失败+rtt 方差大，DialHop 归因必须开 debug；b) TCP 入站被 macOS 防火墙拒绝时直连跳把 refused 直接上抛为最终错误——拨号器应继续尝试其余地址后失败（派 S 核对）。另外确认运维红线：本机为 .15，任何远端/本地 pkill 都会互杀实验节点，采样脚本禁用 pkill。
- 2026-09-02 检查轮 15：四交付全落地验收（D TTL 刷新 2358dfc / D rendezvous keepalive 407e6b9 / T 观测接线 d6b9e6f / S 直连遍历 63116d2）。138 重部署（观测反射口 3402 生效，修复了 systemctl enable --now 不重启进程的脚本缺陷）。E3 第二批实测：观测地址学习生效（218.74.22.62 观测成功）；发现同 NAT 场景「观测地址优先」导致 hairpin 挂 5s 吃满请求预算、LAN 地址未轮到——已派 S 修地址排序（mDNS/同网段优先）。
- 2026-09-02 检查轮 16：S 两连修复落地验收（5696cbf 来源+网段排序、a9be8e2 rendezvous 过滤 loopback、aca31de 同级 QUIC 优先）。E3 采样第三批：LAN 直连稳定 12-13ms；仍存边角失败（v4 观测地址 hairpin refused、发现窗口时序、TCP 防火墙拒绝）——全部立案，属 E4 调优阶段任务，不阻塞 M3 关闭。**E1 通过、E2 完成、E3 打通并完成三批采样**；138 bootstrap 运行最新 main（观测反射口启用）。E4（长稳+采样打磨）为后续阶段。
- 2026-09-02 检查轮 10（E1 首跑）：15/114 双 Mac 节点拉起成功，但 mDNS 互发现失败——本机 debug 复现实锤 D 的 mdns.rs 服务名不合法（缺尾点，mdns-sd 拒绝注册/浏览）。已派急修单回 D（fix/mdns-servicetype）。运维侧两处已修：102 源码快照过期导致编译失败（重同步）、远端节点日志需显式 RUST_LOG=info（runbook 待更新）。
- 2026-09-02 检查轮 10 续：mDNS 服务名修复合入（85a1e1c）后 E1 重跑：**三机互发现成功**（15/114/102 两两可见，含真实局域网地址）、**跨机 ping 成功**（maca→linc RTT 2.75ms）。但断线语义有缺陷：活着的 macb 在首次发现+120s 整被误报断线（mdns-sd 地址集不变就不发 Resolved，TTL 无续期），被 kill 的 linc 反而无断线事件。第二张修复单已派 D（fix/mdns-ttl-refresh：Resolved 恒续期+过期扫描+回归测试）。协调者代 D 修 hostname 尾点（8910de0，D 会话响应滞后，已透明记录）。
- 2026-09-02 检查轮 17：协调权移交新协调会话（session-570fbef3）。E4 调优轮派单：S（拨号可观测性+refused/hairpin 边角，fix/e4-dialhop-observability）、D（发现时序+盲拨降噪，fix/e4-discovery-stability）；长稳采样待 S 交付后派；metrics/gossip 排 E5。规则 5 修正：本仓库远端为 origin（github），收尾恢复 push 步骤。新增 ECS 公网节点（连接信息在 .env，见 ECS_* 条目），用途：第二 rendezvous/relay 兜底与 E4 跨公网长稳，部署派单在 S/D 交付后进行。
- 2026-09-02 检查轮 18：旧 D/T 会话派单后唤醒即转 idle、无开工迹象，判定消息唤醒机制对该两会话失效——对旧 D/T 发停派通知，新开专属会话 p2p-D-E4（session-cc9c45f8，fix/e4-discovery-stability）与 p2p-T-ECS（session-814cafa1，feat/ecs-deploy）承接原单；S（session-e42e5393）确认开工中（.worktrees/e4 编辑 dial.rs）。阿里云安全组 sg-bp1gedk7fadp3vah6vfs 已由协调者经 API 放行 3400/udp、3401/tcp、3402/udp（22/tcp 原已开放）；踩坑记录：DescribeSecurityGroupAttribute 带 Direction 参数触发 InvalidParamter，最小参数集可用。
- 2026-09-02 检查轮 19：用户移除全部旧会话（18 轮"唤醒失效"结论据此更正为会话已被删除，注册表陈旧）。S 被移除前已自行完成 rebase+ff-only 合并（00a775f 归因提升+refused 遍历回归、ff88a8e itest 依赖锁定）但未推远端——协调者接管：push 补齐（c9a24b1..ff88a8e）、worktree 清理、make check 验收 PASS。任务1/2 关闭；任务3（hairpin 快速失败）确认未实现，改派新会话 p2p-S-E4（fix/e4-hairpin-fastfail）。教训：协调者必须以 git 实况而非会话回报为准做验收。
- 2026-09-02 检查轮 20：S（e42e5393）实为存活并回报完整完工报告，与 git 实况完全吻合——18/19 轮"旧会话已全部移除"对 S 不成立（对旧 D/T 成立），验收正式通过（169 用例全绿，协调者补 push + worktree 清理）。语义入册：Direct=false 仅整跳耗尽时发出；同 NAT hairpin refused 已由短地址失败路径快速处理。hairpin 单据此改验证优先（先 itest 复现，达标只交测试）。S 提出遗留：IPv6/跨族监听、QUIC close，登记 E4 余量/E5。在途：D-E4（发现时序）、T-ECS（ECS 部署，worktree 已建）、S-E4（hairpin 验证，worktree 已建 7e4a943）。
- 2026-09-02 检查轮 21：D-E4 验收通过（580f4fd mdns 启动期 1s×5 重询 / d2c3094 退避健康复位缺陷修复 / 6a4df8c 盲拨 WARN 首次制，main=origin/main=538bebb，协调者主树复跑 make check PASS）。两条流程记录：2443366 merge bubble 违反 rebase 规则（不 revert，下不为例）；6a4df8c 跨界改 crates/p2p facade（噪声源确在该层且无并发冲突，豁免，但跨 crate 须先报备）。D 遗留登记：relay_session 盲拨 WARN 不动（无周期刷屏）；真实日志级别断言需 tracing-test 类设施，另立案（E4 余量）。
- 2026-09-02 检查轮 22：S-E4 hairpin 验收通过——分支 rebase 后 0f1c73b ff-only 合入（worktree 门禁+主树 make check 双 PASS，已推远端，worktree/本地/远端分支全清理）。两件套落地：同公网前缀（v4 /24、v6 /64）候选排序殿后 + 2s HAIRPIN_DIAL_TIMEOUT 短预算；itest 断言 LAN 先落地<5s。五条遗留登记为已知限制：CGNAT /24 保守误降权（仍拨仅殿后）、2s 固定值非自适应、纯 rendezvous 无 LAN 注册时只能走 hairpin/中继、itest 黑洞模拟依赖环境路由（book 单测确定性兜底）、book_tests.rs 拆分承接原单测。发现无名 verify worktree（p2p-wt-e4-verify，detached a3a9e95）待归属确认；T-ECS 分支 1 提交（ac9d410 部署脚本 146 行）在途。
- 2026-09-02 检查轮 23：verify worktree 归属确认——系 S-E4 验证优先复现现场，已自清（/tmp 留测量脚本备查）。实测复现入册：hairpin 不通常态为黑洞——单地址挂满预算才失败（QUIC 3.0s / TCP 5.0s 实测），LAN 直连被拖 3-5s；前任 S 的"refused 立即失败"仅对真 RST 成立，按"实测复现缺陷才修逻辑"规则，0f1c73b 修复必要性成立，且其与推送版逐字节一致、主树定向复跑全绿。T-ECS 仍在途。
- 2026-09-02 检查轮 24：T-ECS 验收通过（部署/观测反射/discover 冒烟 PASS，PeerId 入册 runbook §8.5；主树 make check 复跑 PASS，main=e9ff20e）。协调者经阿里云 API 放行 relay 口 3403/udp+3404/tcp。ping 未通定性为产品缺口：CLI 未接线 relay_addrs、--bootstrap 单值、观测首成功恒 v4——据此续派 CLI 接线单（T，feat/cli-relay-wiring）；TCP /t3401 握手后即断为存量缺陷（QUIC 正常），开 p2p-K-E4（session-bf2bc941，fix/e4-tcp-stream）诊断优先承接。观测多反射器/v6 涉及 facade，登记 E4 余量。长稳采样改依赖 CLI 接线（中继兜底就位后采样才有意义）。
- 2026-09-02 检查轮 25：K-E4（新会话）诊断完成——消融实验四步实证根因为 facade TransportLink::connect 丢弃 SecureConn 触发 YamuxMux close-on-drop 自毁（QUIC 因 quinn 驱动任务持有连接幸免；YamuxMux/QuicMux 生命周期语义不一致登记为契约缺口，E4 余量另立案）。协调者裁决采纳方案 A（挂闭包保生命周期），实现派 S 就 K 分支 fix/e4-tcp-stream（含 K 回归 4000845）进行。K 转完成态待命。跨 crate 停手报裁红线首次实战执行，流程正确。
- 2026-09-02 检查轮 26：T-ECS CLI 接线验收通过（363b2a5，双 bootstrap discover 6 peers、relay 会话接线 PASS、DialHop 逐跳打印 PASS；主树 make check 复跑 PASS）。重大定性：E3 的中继兜底从未在真实链路工作——138 ufw relay 口今日才由协调者放行（3403/udp+3404/tcp，密钥登录实测），且 relay 控制流秒断（31/31，~90ms）与 reserve 计入 32 槽配额自锁两缺陷已定位 control.rs/slots.rs/limits.rs。开 p2p-R-E4（session-e5aeee3e，fix/e4-relay-resilience）诊断优先承接，长稳采样改依赖 relay 修复。观测多反射器/v6 维持 E4 余量。
- 2026-09-02 检查轮 28：relay 口端到端验证——nc -vz 实测 15→ECS:3404、15→138:3404、15→ECS:3401 全部可达（两节点 bootstrap 监听确认，p2p-cli 0.0.0.0）。初报"不可达"系协调者探测方法误报（bash /dev/tcp+echo 对即关服务会误判，连 SSH 22 都误报），教训已沉淀：TCP 可达性判定一律用 nc -vz。R-E4 冒烟前置条件全绿。
- 2026-09-02 检查轮 29：R-E4 双缺陷 loopback 100% 复现并定位根因。重大更正：检查轮24 记录的"rendezvous ~5s 生命周期+30s 退避为全系统常态"实为 quic.rs IdleTimeout 单位错（quinn VarInt 单位毫秒，代码传 as_secs()=30 → 30ms）的症状——凡静默>30ms 的 QUIC 连接即被 TimedOut 杀死，relay 控制流秒断、打洞信令丢失同源；mock duplex 测试全绿因不走 quinn。三项裁决：①授权 R 在自己分支代修 p2p-transport 两处 IdleTimeout::try_from（跨 crate 预授权，K 知情，含 >3s 静默存活回归 itest）；②缺陷 b 修复批准——控制流关闭时释放该流上未配对电路，注册载体电路与控制流存亡对齐；③重连退避复位语义（relay_session，S 范围）维持现状——a+b 修复后 churn 自然消失，登记 E4 余量，若长稳采样显示退避欠优再以数据重议。
- 2026-09-02 检查轮 27：长稳采样拆两段——脚本准备先行派 T-ECS（feat/e4-sampling-scripts，执行等 relay 修复）；S 的 TCP 修复实施中（双 worktree，无新提交）；R-E4 消化上下文未现 git 活动，下轮若仍无活动则问询。
