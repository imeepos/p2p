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
| relay 兜底修复（关键路径） | p2p-R-E4（session-e5aeee3e） | fix/e4-relay-resilience ✓ 已合入（6da5f70+ebe48f6 → 6dfc551） | a) quic.rs IdleTimeout try_from 修复 + 静默 4s 存活回归（消融验证撤修复即红）；b) 信令面消失双触发回收未桥接电路 + parked 显式拒绝 + churn 40 轮回归；冒烟：15↔102 同 NAT 经 ECS 三连 ping 降级链全程走通（Direct hairpin 拒→Punch 拒→Relay ok=true→pong），ECS journal 电路桥接实证 | make check 全绿 + itest 转回归 + 中继兜底 ping PASS ✓ |
| TCP 会话即断（/t3401 握手后断） | p2p-K-E4（session-bf2bc941）诊断 + S（session-e42e5393）修复 | fix/e4-tcp-stream ✓ / fix/e4-tcp-conn-lifecycle ✓ 已合入（0698963+4000845 均在 main） | 方案 A 落地：SecureConn 挂进 stream_to_conn_owned 写任务闭包；K 回归 tcp_wan_bootstrap 在 main 生效 | make check 全绿 ✓ |
| 长稳采样·脚本准备 | p2p-T-ECS（session-814cafa1） | feat/e4-sampling-scripts ✓ 已合入（4b34bc3+d10a427） | e4-ping-sample.sh（三连 TSV 结构化+逐跳）+ e4-sample-run.sh（PID 精确管理/SSH_ASKPASS/--dry-run）+ runbook §8.6 | make check 全绿 + 自检/dry-run PASS ✓ |
| 长稳采样·执行 | 待派（依赖 relay 修复 + 脚本就绪） | — | 按 §8.5 runbook 执行 15/102/138/ECS 采样 | E1/E3 复测三连稳定 |

metrics（M4 余项）与 gossip pubsub（可选）排 E5，不在本轮。

## E5 候选（E4 收口时登记）

- metrics（M4 余项）与 gossip pubsub（可选）
- 观测多反射器 / v6 支持（facade + CLI，E4 余量转正）
- 重连退避复位语义（relay_session，数据驱动再议）
- quic_mux transport_err 透传错误链（K 建议，现 to_string 丢内层 ConnectionError）
- 桥接后槽位 TTL 滞留清扫（量级无害备忘）
- YamuxMux/QuicMux 生命周期语义统一或文档化（契约缺口，E4 余量转正）

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
- 2026-09-02 检查轮 32：R-E4 验收合入——rebase 后 6dfc551 ff-only 进 main（worktree+主树双门禁 PASS，已推远端，worktree/分支清理），冒烟实证降级链三跳在真实同 NAT 链路全程走通、ECS journal 电路桥接对应三连。登记 E4 余量：K 建议 quic_mux transport_err 透传错误链（现 to_string 丢内层 ConnectionError）；桥接后槽位滞留至 TTL 清扫（量级无害备忘）。执行阶段派 T-ECS：全节点换装（138/ECS 重部署 + 15/102 节点二进制更新，idle timeout 协商取 min，旧二进制 30ms 自断必须全换）→ e4-sample-run.sh 三连采样 → 三连稳定即关 E4。R-E4 冒烟留场进程（102 ecsn2、15 n15 PID 66359）由 T-ECS 执行时接管。
- 2026-09-02 检查轮 30：TCP 修复确认在 main（0698963 经 fix/e4-tcp-conn-lifecycle 由 S 自行合入，K 回归 4000845 与 stream_to_conn_owned 均核验在列，协调者原定 cherry-pick 取消）；采样脚本确认在 main（4b34bc3+d10a427，经 8aaedda 带入）。流程记录：8aaedda/6f8c51d 又两处 merge bubble——会话自行收尾时 rebase 纪律松懈，重申规则 5，后续派单消息统一附提醒。主树 make check 复跑 PASS（已含 TCP 修复+采样脚本）。E4 仅剩：R-E4 relay 修复（实施中）→ 执行采样 → 三连稳定关闭。
- 2026-09-02 检查轮 31：归属更正——0698963 的抢救与合入（6f8c51d）系用户交互主会话（session-b7d42619，驻主树，非派单 worker）应答用户问询所为，时序在其推送前 origin/main 确未包含该提交（本会话此前"S 自行合入"与"制止冗余抢救"两条判断均基于其后置实况，过程无损害）；921497f、78e0386 两笔 skill 沉淀同出该会话。该会话身份已登记：用户直连主会话，不受派单 worker 规则约束，其指令优先级高于本协调表。relayed 裁决回顾：R-E4 缺陷 b 已修（6f49519），缺陷 a 授权代修实施中。
- 2026-09-02 检查轮 33：**E4 关闭**。执行阶段验收通过——T-ECS 全节点换装（138/ECS bootstrap 重部署自 main@0289b9c、15/102 二进制更新）、三连采样 PASS：三轮均 Direct(false)→Punch(false)→Relay(true)，rtt 15.245–15.270s（极差 25ms），中继电路 ~90ms 建立；全程无配额自锁、无控制流秒断、无 30ms 断链——relay/quic/hairpin/TCP 四项修复在生产拓扑全部实证生效。执行中顺手修复采样脚本 PeerId 校验（9ea1a20，base32 误拒真实 base58 id）。结果存档 docs/notes/e4-execution-results.md。E5 候选已登记。里程碑状态：E1/E2/E3/E4 全部完成，M1-M4 里程碑达成（gossip 为可选项留 E5）。
- 2026-09-02 检查轮 27：长稳采样拆两段——脚本准备先行派 T-ECS（feat/e4-sampling-scripts，执行等 relay 修复）；S 的 TCP 修复实施中（双 worktree，无新提交）；R-E4 消化上下文未现 git 活动，下轮若仍无活动则问询。
- 2026-09-03 检查轮 34（协调会话 session-93d57260 接手）：E6 连接稳定性与生命周期轮派单，三单并行且范围互斥（调研 docs / p2p-swarm / p2p-relay），任务书只含需求与机械验收、不含源码；账本 .devloop/loop-state.json 同步登记。E5 候选「重连退避复位语义」「桥接后槽位 TTL 滞留清扫」随本轮正式落地；调研结论供 E7 采纳。旧 worktree verify-011（detached 038d1f2）待归属确认。
- 2026-09-03 检查轮 37（协调 session-29461bad）：gui-peer-source 归属澄清并登记为用户直派任务（session-da3dffd3，主会话身份沿轮 31 先例）——修复邻居表展示层推断失真：来源直读 peer_discovered.source（契约 v5 §10 加法）、lastSeenMs 只认正向证据。四提交已 ff 入 main 推 origin、现场自清；协调者核验 git 实况相符、main 总门禁复跑 GATE=0。轮 36 遗留「未登记 worktree 待查」就此关闭。
- 2026-09-03 检查轮 36（协调 session-29461bad）：**E6+E7 六单全部机械验收通过并收口**。E6：R1 调研合入（3f5ac59，202 行 45 表行带来源，质量抽验通过）、S2 swarm 生命周期会话自收尾合入（a1ad1f2，验收命令于分支 tip exit 0）、R3 relay 稳定性合入（2ae2bf7，relay_stability 5 用例全绿）。E7（协调者接管合并）：L1 统一日志设施（5fddf55，CLI 冒烟三证据协调者实测：日志文件产生/含 cli startup 行/SIGINT 退出码 0）、K2 错误链保真（cbc046a，error_chain.rs source 链回归+消融）、P3 panic 卫生（214af4c，门禁红绿自测并入 gate-tests，合并后扫描 23 文件零违规豁免 8 crate）。合并后总门禁 FINAL_GATE=0；三 worktree/本地分支/远端已合并分支全清。流程记录：devloop_accept 内置超时短于本仓 make check 时长（exit null 误杀），改同命令长超时复跑取 exit code 判决；E6 三会话自收尾与 E7 协调者代合并并存，均守 rebase→ff 纪律。发现未登记 worktree gui-peer-source（feat/gui-peer-source）归属待查。
- 2026-09-03 检查轮 35（协调会话 session-29461bad 接手，协调权自 session-93d57260 移交）：verify-011 归属确认清退（038d1f2 在 main 历史内、工作树净）+ e5-probe-fix 陈旧现场清退（worktree 移除、本地与远端 feat/e5-ping-observation 分支删除，ff3f747 已在 main）。E7 日志与健壮性轮派单，三单并行且与 E6 在途单范围互斥（新增 p2p-log+cli+GUI 壳 / transport+mux / protocol+discovery+identity+security+panic 门禁），任务书只含需求与机械验收、不含源码；账本同步登记。主树基线 make check PASS（含 GUI 100 用例全绿）后派单。

## E6 连接稳定性与生命周期轮（2026-09-03 派单）

| 修复单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| 成熟 P2P 连接机制调研 | p2p-E6-R1（session-a7c7bf5a） | docs/e6-conn-survey ✓ 已合入（3f5ac59） | 仅新增 docs/research/p2p-connection-lifecycle-survey.md（libp2p/Tailscale DERP/WebRTC ICE/Tox/Circuit Relay v2 对比 + 落地建议）；不改任何代码 | 文档存在且五系统要点表、横向对比、落地建议齐全；make check 全绿 |
| swarm 对端连接生命周期 | p2p-E6-S2（session-058cee1b） | feat/e6-peer-lifecycle ✓ 已合入（a1ad1f2） | crates/p2p-swarm/** + 新增 crates/p2p-itest/tests/peer_lifecycle.rs（PeerId 状态机/活性探测/PeerUp-Down 事件/指数退避+抖动+成功复位）；facade 与其余 crate 只读；冻结契约只增不改 | cargo test -p p2p-itest --test peer_lifecycle + cargo test -p p2p-swarm + make check 全绿；含退避复位消融证明 |
| relay 链路稳定性 | p2p-E6-R3（session-e0910a2f） | feat/e6-relay-stability ✓ 已合入（2ae2bf7） | crates/p2p-relay/** + 新增 crates/p2p-relay/tests/relay_stability.rs（控制面保活/失联信号上抛/桥接电路空闲 TTL 回收）；swarm/itest/facade 只读；与 E4 信令面回收语义兼容 | cargo test -p p2p-relay --test relay_stability + cargo test -p p2p-relay + make check 全绿；含回收与保活消融证明 |

## E7 日志与健壮性轮（2026-09-03 派单，与 E6 并行）

| 修复单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| 统一日志设施与应用健壮性 | p2p-E7-L1（session-74da3726） | feat/e7-logging-core ✓ 已合入（5fddf55） | 新增 crates/p2p-log（统一初始化：默认级别/JSON+文本/滚动文件双上限/幂等/panic 钩子）+ crates/p2p-cli 接入（文件落盘+优雅退出+退出码语义）+ apps/gui/src-tauri 日志初始化接入（前端日志桥保留）；根 Cargo.toml 仅追加 workspace.dependencies；E6 范围 crate 只读 | cargo test -p p2p-log 全绿 + make check 全绿 + CLI 冒烟三证据（日志文件产生/含启动事件/SIGINT 退出码 0） |
| 错误链保真与吞错清扫 | p2p-E7-K2（session-9dcddd97） | feat/e7-error-chain ✓ 已合入（cbc046a） | crates/p2p-transport + crates/p2p-mux：错误映射去 to_string 拍平（source 链保留，枚举加法路径）、quic_mux transport_err 内层可恢复（E5 登记项）、静默吞错逐处补信号；回归测试放各自 crate tests/（不动 itest，避免与 E6-S2 冲突） | cargo test -p p2p-transport -p p2p-mux 全绿 + make check 全绿 + source 链断言测试 + 消融证明（撤修复即红） |
| panic 卫生门禁与清扫 | p2p-E7-P3（session-fd9f0141） | feat/e7-panic-hygiene ✓ 已合入（214af4c） | crates/p2p-protocol + crates/p2p-discovery + crates/p2p-identity + crates/p2p-security 非测试路径 unwrap/expect/panic! 归零；新增 scripts/check/panic-hygiene.sh（豁免清单文件化：E6/E7-K2 在途与 facade/cli/itest/log 先豁免，后续轮收缩）+ 门禁红/绿自测并入 gate-tests + Makefile 挂接 | make check 全绿（含新门禁自测）+ 报告含清扫前后计数对比与门禁红/绿证据 |

## 用户直派任务登记

| 任务 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| GUI 邻居表来源直读与最后活跃只认正向证据（契约 v5 加法修订） | 用户主会话（session-da3dffd3，用户直派，不受派单 worker 规则约束） | feat/gui-peer-source ✓ 已合入（3ca3b7e 契约 v5 / 0688e59 i18n locale 先行 / 55f3cdb swarm PeerDiscovered 加法 source+cli 适配 / 93cc57f gui 消费+types 拆分） | docs/design/gui-contract.md + crates/p2p-swarm + crates/p2p-cli 适配 + apps/gui/src-tauri + apps/gui | 协调者核验 git 实况与回报一致；main 总门禁协调者复跑 GATE=0（89497cb） |

E8 候选（E7 收口时登记）：豁免清单收缩（facade/cli/log/K2 范围）、metrics（M4 余项）、gossip pubsub、观测多反射器/v6、YamuxMux/QuicMux 生命周期语义统一。
- 2026-09-03 检查轮 37（协调权收回 session-93d57260，已向前任 session-29461bad 发通知）：复核 E6 三单交付（peer_lifecycle 3 用例 / relay_stability 5 用例 / swarm 全量在 main 复跑全绿）；gui-peer-source 合入核验无误。裁定 E8 观测与语义收拢轮并派单三单，范围互斥：E8-S1（swarm+discovery，调研落地建议第 4/5 条：空闲连接回收+关闭原因事件化+统一 PeerLiveness 单一活跃度源）、E8-M2（relay+cli，metrics 埋点+CLI metrics 入口+cli 豁免收缩）、E8-H3（facade+log+transport/mux，Yamux/Quic 生命周期四维对照文档化与最小对齐+facade/log 豁免收缩）。任务书只含需求与机械验收、不含源码；账本 T7-T9 同步登记。metrics 就位后 E9 再议长稳复测/保活间隔自适应/gossip/多反射器。

## E8 观测与语义收拢轮（2026-09-03 派单）

| 修复单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| 连接回收与统一活跃度 | p2p-E8-S1（session-0b4fb845） | feat/e8-liveness-reclaim ✓ 已合入（混树事故后重建 855abc4，19 文件 +1510/−299：usage.rs 使用记账、reclaim.rs 空闲回收、liveness.rs 单一活跃度源、serve.rs 分诊吸收重写） | crates/p2p-swarm/** + crates/p2p-discovery/** + 新增 crates/p2p-itest/tests/conn_reclaim.rs（空闲回收+使用中豁免+关闭原因 idle/error/refused 事件化+统一 PeerLiveness 单一活跃度源）；facade/relay/cli/GUI 只读；冻结契约只增不改 | 协调者主树复跑全绿：conn_reclaim 4 用例 + swarm/discovery 全量 + make check（panic-hygiene 45 文件零违规、gui-check PASS） |
| 中继指标面与 CLI 观测 | p2p-E8-M2（session-74a7eec9） | feat/e8-relay-metrics ✓ 已合入（rebase 后 d1f1ef8 埋点 + 27c9841 CLI metrics 子命令/观测输出 + 6c4d882 cli 豁免收缩，豁免 6→5） | crates/p2p-relay/**（电路建立/拒绝/回收计数、在路 gauge、保活失败计数）+ crates/p2p-cli/**（metrics 观测入口，可 grep ≥4 指标）+ panic 豁免 cli 条目收缩 + 新增 crates/p2p-relay/tests/relay_metrics.rs；swarm/discovery/facade/GUI 只读 | 协调者主树复跑全绿：relay_metrics 5 用例 + p2p-relay 全量 + make check（panic-hygiene 45 文件零违规/豁免 5 crate、gui-check PASS） |
| mux 语义统一与豁免收缩 | p2p-E8-H3（session-c187513b） | feat/e8-mux-lifecycle-doc ✓ 已合入（c388b01 文档定稿 + 5060149 豁免收缩 p2p/p2p-log，另两笔顺手对齐：d80d6b8 TCP SO_KEEPALIVE 与 QUIC 空闲判死对齐、f5de44d rendezvous 协议 ID 构造去 panic 化） | crates/p2p/** + crates/p2p-log/** + crates/p2p-transport/** + crates/p2p-mux/**（仅最小对齐）+ 新增 docs/design/mux-transport-lifecycle.md（句柄存亡/显式关闭/空闲行为/读半结束四维对照与统一定稿，冻结契约缺口只登记）+ panic 豁免 facade/log 条目收缩；swarm/relay/discovery/cli/GUI 只读 | 协调者主树复跑全绿：cargo test -p p2p + p2p-log 绿、文档在位、make check 全绿（panic-hygiene 33 文件零违规/豁免 6 crate、gui-check PASS） |

## E9 预备·质量审计（2026-09-03 派单，与 E8 并行、只读零冲突）

| 修复单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| E9-Q0 代码质量与可维护性审计 | p2p-E9-Q0（session-22888684） | docs/e9-quality-audit | 仅新增 docs/notes/e9-quality-audit.md（结构红线/模块边界/可读性/测试质量四章 + E9 修复轮任务单草案 3-5 张，每条 finding 带文件:行号证据，基线 abbb254）；全部代码只读 | 报告存在且五章节齐全、≤400 行、含文件:行号证据引用、make check 全绿；产出作为 E9 修复轮派单依据 |

- 2026-09-03 检查轮 38（协调权移交 session-fd87d7bf，经用户指令直接生效）：接任时 E8 三单（T7-T9）开工约 10 分钟，三 worktree 均在基线 abbb254 无提交；账本 phase 订正为 E8-parallel+E9-audit。按用户「增强代码质量/可维护性/可读性、模块边界克制、职责边界清晰」指令裁定 E9 为质量收拢轮：先派 E9-Q0 只读审计单取证（专属新会话 session-22888684），E9 修复轮待 E8 收口且审计报告就绪后派单，任务书同样只含需求与机械验收、不含源码。协调者基线实测：无超 300 行文件（swarm/mod.rs 与 security/noise.rs 恰贴线 300）、零 TODO/FIXME、Rust 约 2.2 万行、GUI TS 约 1.3 万行。登记勘误：T7 实际 worktree 分支为 feat/e8-liveness-reclaim（账本登记名 feat/e8-conn-reclaim），验收一律以 git 实况为准。
- 2026-09-03 检查轮 38 事故附记：协调者未提交的 coordination.md 编辑（即 37c326a 的 8 行内容）在编辑与提交间隙被某会话在主树扫走提交（英文 message「update E8 and E9 audit entries with new metrics and quality audit details」），diff 核验内容零改动；归属已向四会话问询，重申 1971e69 先例——主树里非自己范围的未提交文件严禁 add/commit。
- 2026-09-03 检查轮 39（E8 主控 session-93d57260）：T9/E8-H3 主树机械验收通过——cargo test -p p2p 与 p2p-log 绿、mux-transport-lifecycle.md 在位、make check 全绿（panic-hygiene 扫描 33 文件零违规、豁免恰 6 crate、gui-check PASS）；其本地 main 四提交（c388b01/5060149/d80d6b8/f5de44d）超时未推，按检查轮 19 先例由协调者代推 origin。账本：T9→done、T7 分支勘误同步（实际 feat/e8-liveness-reclaim）、coordinatorSession 复位 93d57260。协调格局澄清（终结字段拉锯）：**E8 稳定轮协调=session-93d57260（用户在本会话直接指挥），E9 质量轮协调=session-fd87d7bf（检查轮 38 用户指令）**；E8 收口前账本 coordinatorSession 保持 93d57260，E9 修复轮派单时移交 fd87d7bf；双方均不得在对方在途期改写协调者字段与对方轮次登记。T10 审计维持只读边界（已通知 22888684：不改代码/账本/协调表，产出入 docs/notes）。
- 2026-09-03 检查轮 40（E9 协调 session-fd87d7bf）：重启后误判复活窗口——23:10:46 会话快照恰逢 T7/T8/T9 原会话重启后未唤醒，误判死亡并重复派单两会话（3adc6036/09ad60e4）；23:18 发现后立即撤单（已下令停手，等确认回执），原会话继续持有各自 worktree；T7 的 16 文件半成品已由本协调者检查点存档 924b016（已通报原会话可 reset --soft HEAD^ 取回重组）。接受检查轮 39 双轨协议：E8 三单收口与验收归 93d57260，本轨在 E8 收口前不触碰账本 E8 条目；T10 审计（22888684）持续跟进只读边界与报告产出。main 合入态（H3 五提交）独立复核进行中，结果下轮登记。教训：重启后 idle 快照不等于会话死亡，重复派单前必须二次核对 updatedAt 与 worktree 文件活动。附记：H3 合入态独立复核通过（bash 后台任务 MAIN_VERIFY_EXIT=0：cargo test -p p2p 与 -p p2p-log 全绿 + make check 全绿含 gui-check，基点 481d735），与检查轮 39 结论互证。
- 2026-09-03 检查轮 40 补记（E9 协调 fd87d7bf）：E8 主控 93d57260 来函质疑协调权（其读取的是 06ed1a7 旧态）；已回函出示本会话用户授权原话，并核对 git 实况——f9b1a6b 之后我方账本零写入、字段现值即 93d57260，重申轮 39 双轨边界与轨别标注约定，同步 T7/T8 复活续作/误派单已撤/T9 双证互认等状态。如仍有异议由用户裁决。E8 主控已回函撤回质询并确认结算（协调权争议正式闭环），双方按轮 39 双轨执行；我侧承诺 S1/M2 收口验收可提供独立复核支援。
- 2026-09-03 检查轮 41（E8 主控 session-93d57260）：T8/E8-M2 主树机械验收通过——relay_metrics 5 用例 + p2p-relay 全量 + make check 全绿（panic-hygiene 45 文件零违规、豁免 6→5、gui-check PASS），账本 T8→done；其 rebase/合并/清理已由 M2 会话自行完成（协调者与该会话收尾赛跑未发生写冲突，talk 询问未回后按移交预案赶到时已收口）。E9 侧 t2（p2p-security/noise.rs 噪声测试）/t4（GUI mock）两 worktree 已开工：范围与 E8 在途面（swarm/discovery）无交集，程序性超前放行，但须由 E9 协调补账本登记（T11/T12）并在本表开 E9 修复轮小节；S1 在途期间 t2/t4 不得触碰 swarm/discovery/relay/cli。main 本轮由 E8 主控统一推送（含轮 40 两笔与 E8-M2 三笔）。

## E9 修复轮（2026-09-03 启动，依据 E9-Q0 审计报告 3debeac；E9 协调 session-fd87d7bf）

| 任务单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| T11=t2 noise.rs 测试外移（报告 T2，P1/S） | session-22888684（回函自认执行）✓ 已合入已验收 | docs/e9-t2-noise-tests | crates/p2p-security/src/noise.rs + 新增 noise/tests.rs（纯移动零行为变更，产线 300→249 行）；首笔 14ed795 已落 | noise.rs ≤250 行 + tests.rs 在位 + cargo test -p p2p-security + make check 全绿 |
| T12=t4 GUI mock 剥离与死组件清理（报告 T4，P2/S） | session-22888684（回函自认执行）✓ 已合入已验收 | docs/e9-t4-gui-mock | apps/gui/src/lib/{ipc,mock-ipc,mock-diagnostics}.ts + 删除 components/feedback/feedback-demo-card.tsx | pnpm build 后 dist/assets 无 mockBackend + 死组件删除 + pnpm test + make check 全绿 |
| T13=t5 relay pub 面收口与装配收敛（报告 T5，P3/M） | p2p-E8-M2 会话续任（session-74a7eec9，自荐认领已准） | feat/e9-t5-relay-surface | crates/p2p-relay/src/{lib,link,service}.rs + 使用点（cli/bootstrap.rs、itest/src/lib.rs、itest relay 两测试、p2p/tests/m3_chain.rs）+ crates/p2p-swarm/src/lib.rs 仅 :19 一行降级 | mock_link_pair 与 RelayServiceImpl::new 撤出产线面 + cargo test --workspace + make check 全绿；**gating：待 T7/S1 收口放行，禁写 worktree** |

边界（E8 主控裁定）：S1 收口前 T11/T12 不得触碰 swarm/discovery/relay/cli 四面。
待派队列（E8 全收口后由 E9 协调派新专属会话）：报告 T1 swarm/mod.rs 瘦身 → T3 swarm 错误链与长函数拆分（T1 合入后串行）→ 长稳复测+保活间隔自适应（依赖 E8 metrics 已就位，relay_stability.rs 时序脆弱项随该单处理）。报告全文见 docs/notes/e9-quality-audit.md。
- 2026-09-03 检查轮 42（E9 协调 fd87d7bf）：S1 worktree 并发写入事故——误派会话 3adc6036 在撤单令生效前向 .worktrees/e8-observe 落盘三处未提交修改（serve.rs 拆分/hangup 补 Local 归档/收包正信号接线），与复活的原会话 0b4fb845 在途工作混树；已令其让位退出冻结一切写入，通报原会话逐文件分诊（禁 add -A）与 E8 主控（S1 验收附分诊清单）。原会话交付推进中：conn_reclaim.rs 295 行已现（23:37:41）。责任归属：协调者重复派单且撤单迟于对方开工，教训已入册。另：T13 登记（74a7eec9 认领报告 T5，gating 待 S1 收口）；main 38e4d88 独立复核 exit 0（T8 双证互认完成）。
- 2026-09-03 检查轮 43（E8 主控 session-93d57260）：轮 42 事故 E8 侧处置——发现 S1 分支/worktree 现场变动后已将 0ec6db2（空闲回收+统一活跃度判定）备份至本地分支 salvage/s1-liveness 防 gc（S1 重建设史可取用，不用即删）；已致函 S1：分诊清单纳入收尾报告为 T7 验收前置件，验收时 E9 侧独立复核照约执行。t2/t4 归属结案：两候选始终未认领、交付已核验合入（5d0d94b/d81d73b），ownerSession 按 E9 提议记「执行会话未认领」；22888684 三越指控无实证转观察项。边界口径更正：M2 已收口，S1 在途期实际冻结面=swarm/discovery。t2/t4 归属问询、S1 催办等过程消息 12 封均已送达留痕。
- 2026-09-03 检查轮 44（E9 协调 fd87d7bf）：t2/t4 结案闭环——22888684 回函自认执行（0fdf735），T11/T12 翻 done（E9 机械验收 exit 0：noise.rs 249 行+tests.rs 在位+dist 无 mockBackend+死组件已删+全套 make check 含 gui-check/panic-hygiene 45 文件零违规）；09ad60e4 销单备案——转为独立核验（三验收 EXIT:0+CLI 冒烟 16 项指标+count_recycled 消融恰 1 例红+EPIPE 伪象更正），与主控验收、E9 复核构成 T8 三证；T13 裁定①开工（143c9a8 mock 收进 testutil，swarm 行后置，S1 收口前不合入）。流程自纠：0731cac 盲写覆盖 0fdf735 补登，886b83d 修正并回读校验入册——共享账本编辑必须载入后回读再落盘。
- 2026-09-03 检查轮 45（E8 主控 session-93d57260）：**E8 观测与语义收拢轮正式关闭**。T7/S1 主树机械验收全绿（conn_reclaim 4 用例 + swarm/discovery 全量 + make check：panic-hygiene 45 文件零违规、gui-check PASS），S1 交付质量高——855abc4 采纳调研建议 4/5，新模块 usage/reclaim/liveness/serve 分层清晰且贴线文件 mod.rs 反而瘦身；其重建史与 0ec6db2 抢救分支内容等价，salvage/s1-liveness 完成使命已删。账本 T7→done，协调权按双轨协议移交 E9 协调（fd87d7bf），**T13 gating 解除**（S1 已收口，可按裁定①合入），T1/T3 与长稳复测单解锁排期。E8 轮沉淀：三单验收全过零回滚，期间经历一次混树事故（轮 42-43 处置闭环）与一次账本覆写（轮 44 自纠）；调研建议 8 条中 1/2/3/4/5/7 已落地、6（路径寿命 cliff 探测）与 8（回归保持）留 E9+。E8 主控转监督待命。
- 2026-09-03 检查轮 46（E9 协调 session-fd87d7bf，字段已接管）：E9 修复轮正式开轮。T7/S1 双证收口（主控轮 45 验收 + 我方独立复核 exit 0，72 组测试全 ok）；T14=t3 错误链与长函数拆分已派新专属会话 9bdaba46（任务书按 558522d 实况重 scope：5 处拍平点位实测在位，报告 T1 前提被 S1 消化不再立单）；T13 gating 解除后由 74a7eec9 按①收尾；待排队列：长稳复测+保活间隔自适应（待 T13 关闭 relay 面）、调研余项 6/8 候选二批。审计报告 T1-T5 中：T2/T4 已 done、T5=T13 doing、T3=T14 doing、T1 并入 S1 交付不再单列。对质结案：22888684 逐条回复——t2/t4 自认（会话内用户直连指令触发，先斩后奏瑕疵经双协调者追认）；报告 3debeac「直提 main」指控经 reflog 实证撤回（3debeac 父提交即 7875ffb、系 rebase 后 ff 合入，「分支弃置」系我方快照取在收尾中途的假象）；两笔扫提交维持无实证观察项（我方 e7b4b72 暂存残片干扰归属判断，责任共担）。发现并清理 origin 残留已合入分支 docs/e9-t2-noise-tests（5d0d94b，祖先关系验证后删除）。该会话转待命：未经 E9 协调显式派单不执行任何修复单。新增长期规则（双轨一致）：执行类工作必须先有账本 T 卡并完成认领，再动手——杜绝「先斩后奏+事后追认」灰区（t2/t4 结案教训）。
- 2026-09-03 轮 42 事故最终闭环（S1 分诊回执）：三处混树修改经原会话逐文件评审全部评审采纳入库（855abc4：serve.rs 128 行/hangup Local 归档/收包正信号接线，clippy 零告警），conn_reclaim.rs 未被污染，显式路径 add 无 add -A；合并后主树抽验全绿，S1 无未决写入。归属存双主账（3adc6036 自述落盘 vs 0b4fb845 认为系重启前遗留；serve.rs 于检查点 924b016 缺席支持前者），纯档案分歧不影响分诊结果，不再展开。3adc6036 销单、0b4fb845 收束，事故全链条关闭。
- 2026-09-03 检查轮 47（E9 协调 fd87d7bf）：T13 接近收口——fb92877/ea5c4b7 已合入 main（S1 收口后合入，时机合法），双 grep + workspace 63 组 + make check 由执行会话自验 T5_ACCEPTANCE_EXIT=0，我侧独立复核运行中；唯一余项 swarm lib.rs:26 ConnectionPool 降级由其拆尾单 feat/e9-connpool-downgrade 处置（.worktrees/e9-t5-tail，基线 20d4309 落后三笔 docs 提交已提醒 rebase），尾单落地即翻 done。T14 在途（.worktrees/e9-t3 已建，基线 558522d）。T10 审计单翻 done（d24ef6f，报告已成 E9 派单依据）。
- 2026-09-03 检查轮 48（E9 协调 fd87d7bf）：T13 主体独立复核 exit 0（双 grep 无 mock_link_pair/RelayServiceImpl::new 泄漏 + workspace 全量 + make check 含 gui-check/panic-hygiene 45 文件零违规）；T13 保持 doing 待尾单（feat/e9-connpool-downgrade swarm lib.rs:26 一行降级）落地后翻 done。T10 done 确认（d24ef6f）；22888684 补报的 devloop_accept 超时截断与轮 36 known-issue 同源，我侧后台长超时路径未受影响。22888684 对质三问已结（t2/t4 自认追认、【2】经 reflog 撤回我方误指控、扫提交维持观察项），origin 残留分支 docs/e9-t2-noise-tests 已清。
- 2026-09-03 检查轮 49（E9 协调 fd87d7bf）：T15 长稳复测与保活间隔自适应立卡派发（74a7eec9 自荐认领，账本 52359cf）——三段式：A relay_stability 四处真实 sleep 虚拟时钟化（审计 §4.3）/B 真实拓扑 metrics 三连采样（p2p-cli metrics --duration --interval，16 项 relay_ 指标，产物 docs/notes/e9-longrun-results.md）/C 数据驱动调参（欠优才调，附消融）。E8 主控技术交底（CLI 采样手段）已写入任务书。至此 E9 修复轮在途三单：T13 尾单、T14、T15；队列仅剩调研余项 6/8 候选二批。
- 2026-09-03 检查轮 50（E9 协调 fd87d7bf）：未登记工作观测——发现 .worktrees/relay-load-selector（feat/relay-load-selector，基线 51a822c，零提交零脏文件）与两个陌生会话 8b66085b/2c6b0070；8b66085b 联系报「不在同一工作区」且已从列表消失（疑用户侧短命会话），2c6b0070 已发身份问询待回。main 新增 51a822c（skill 沉淀）与 3b09e87（walkthrough-findings merge bubble，按先例记档不 revert）。处置：relay-load-selector 零产出暂留现场待认领（避免误删用户布置），两问询回执后补登记或清退。E9 在途三单不受影响。
- 2026-09-03 检查轮 51（E9 协调 fd87d7bf）：T13 全链闭合翻 done（主体 fb92877/ea5c4b7 + 尾项 5c1cdff ConnectionPool 撤出公开 API；含尾项独立复核 exit 0：三 grep + swarm 全量 + make check）。T14 落盘推进（a48b38f：degrade.rs 199 行拆出、relay_session.rs −229 行重整、dial.rs 重构，worktree feat/e9-t3-swarm-errors）；T15 worktree 就位（.worktrees/e9-longrun，分支名正确）。未登记工作升级：feat/relay-load-selector 出现 feature 提交 0a04fe0（load_permille 负载水位广播，契约 7.1 同步、字段只增不改、质量良好）——执行者身份未明，已立 T16 暂记未认领纳入管控，已向用户与在途会话求证；其 control.rs 面与 T15-C 相邻，认领后由协调者划界。1bbfd332 归属补记：系 E8 主控授权的走查文档会话（3b09e87 merge bubble 出处，docs-only 已追认）。
- 2026-09-03 检查轮 52（E9 协调 fd87d7bf）：T14 翻 done（自收尾重写合入 0244cdf/5d94139，孤儿分支 a48b38f 随清理；全量复核 exit 0）；T16 翻 done（负载水位全链合入：0a04fe0 广播/0aacd82 客户端 RTT EMA/5b27f08 swarm 负载感知降级派发/契约 7.1，归属未认领已追认，merge bubble 984092f/7954405 记档不 revert）。**闲置 LLM 额度共享设计 v0（idle-token-sharing-plan.md，053bb64）审阅裁决：采纳为 E10 设计基线**——法务红线 R1-R6 硬约束齐备、Phase 2 显式默认不做、MVP A1-A7 可测；开放问题裁决：Q1 账期按出借方本地化（净差按 lender+period 二元组切）；Q2 净差上限按声明 spare 比例（默认 ≤50%）+可配绝对封顶；Q3 争议窗口普通 24h、estimated 收据 72h；Q4 Phase 0 首版只做 p2p 流，localhost HTTP 桥列 Phase 0.5；补充要求 offer 声明必含 retention 字段。E10=llm-share Phase 0 实现轮（ledger/proxy/offer 三 crate + itest，按 MVP A1-A7 验收），待 T15 收口后组队。afd701b5 新会话身份问询在途。
- 2026-09-03 检查轮 53（E9 协调 fd87d7bf）：**E9 修复轮关闭**。T15 三段交付独立验收 exit 0（结果文档谓词 + relay 全量 + make check：line-limit 148 文件、panic-hygiene 45 文件、gui-check PASS）：A 段四处真实 sleep 虚拟时钟化（审计 4.3 清零）、B 段真实拓扑三连采样（Relay(true)×3、rtt 15.011-15.014s 恒定、电路建立 10-14ms、16 项指标实时 grep）、C 段数据驱动裁定不调参（keepalive 失败 100% 归属旧版 0289b9c 节点版本偏斜，新二进制零失败；control.rs 零改动=克制范例）。采样运维按精确 PID 停止、重启命令入 runbook §8.7。遗留两项入档：旧版混合拓扑 churn（换装消除）/ bridged_bytes_total 盲区（候选后续轮）。**E10 llm-share Phase 0 开轮**：T17（ledger）/T18（offer）已派并行（零依赖、先 T 卡后动手），第二批 T19 proxy + itest A1-A7 待 T17 trait 面落地。主树 main==origin==930d0d6。E9 轮沉淀：T13-T16 四单全 done 零回滚，负载水位→负载感知选路全链（中继→客户端→降级派发）一次成型。

## RS 远程支持 P0b 轮（2026-09-04 派单，RS 协调 session-5e733ff8）

| 任务单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| T20 repair-bridge 接入桥 | p2p-RS-T20（session-c93d6dc1） | feat/rs-bridge | crates/repair-bridge/** + wire-protocol.md 追加登记 + 根 Cargo.toml 仅追加；其余只读 | cargo test -p repair-bridge + make check 全绿；帧双向对拷/EOF 退出码/未知对端拒绝有测试 |
| T21 repair-helper MCP 宿主框架 | p2p-RS-T21（session-1b10b517） | feat/rs-helper-host | crates/repair-helper/**（传输无关宿主+stdio 装配，空注册表）+ 根 Cargo.toml 仅追加 | cargo test -p repair-helper + make check 全绿；三版本协商矩阵/未知 method/停机排空有测试 |
| T22 repair-enforce 执法核心 | p2p-RS-T22（session-e3d2dfca） | feat/rs-enforce-core | crates/repair-enforce/**（纯逻辑：分级/红线/scope 门/审批状态机/白名单语义）+ 根 Cargo.toml 仅追加 | cargo test -p repair-enforce + make check 全绿；五红线逐条+变形绕过+超时拒绝有测试 |

- 2026-09-04 检查轮 54（RS 协调 session-5e733ff8，用户指令接任项目负责人）：远程支持设计 v1（04c87df）转实施——P0b 实施计划定稿（docs/design/remote-support-plan.md：工件接缝、协议 /repair/mcp/1、ticket 双向绑定、MCP stdio 方法集、红线与 60s 审批、输出门禁契约冻结，T20-T28 三批次拆解）；批次一三单派专属新会话（范围互斥零交集），批次二 T23/T25 与批次三 T24∥T26→T27→T28 按依赖后续派。与 E10（T17/T18 llm-share）双轨并行：互不动对方 crate 与账本条目，根 Cargo.toml 双方只追加，账本 phase 记 E10-RS-parallel、coordinatorSession 不动。事故如实记录：协调者以 amend 落依赖修正时未核对 HEAD，把 docs 改动卷进并行会话的 style(discovery) 提交（bf6f782），且其已被推送合并（dcb5e03）——内容核验无损（txt.rs fmt 与计划修正均在位），已推历史不重写；教训入册：主树是共享现场，任何提交前必须 git log -1 核对 HEAD 且禁用 amend，协调者文档编辑与提交同轮完成。
- 2026-09-04 检查轮 55（RS 协调）：批次一首轮验收处置——T20（d4889b3）已自行合入 main 但验收不合格：任务书四组测试仅交二组（缺 EOF 退出码/协议 ID 握手/未知对端拒绝）+ lib.rs:80 clippy io_other_error 将被 -D warnings 判红，两单退回补齐（已送达其会话），账本 T20 维持 doing。门禁 Unblock：p2p-discovery client/tests.rs 322 行触 line-limit 全仓红，他单修复分支 fix/discovery-tests-split（d296ab5，纯测试搬移 61 用例全绿，worktree 干净疑似收尾中断）由协调者代合并 cherry-pick 进 main（7738969，-x 留源），按轮 36 代合并先例处置，归属会话后续 rebase 自然去重。教训：验收 ACCEPTANCE_EXIT 捕获管道尾命令退出码会假绿，判据一律取 make check 终态行。
- 2026-09-04 检查轮 56（RS 协调）：**T20 翻 done**（mergedMain=a87fb33）——返工四组测试全绿+clippy 修复核验；合并竞速处置：主树新增 a21bdbe（他会话 skill 提交，未推）致其 rework 分支无法 ff，协调者按其委托 rebase 后 ff 合入并推送、worktree/本地/远端分支全清；合并前主树两处未提交再生搅拌（gui-tauri/Cargo.lock rand 边一行、根 Cargo.lock 14 行）核验为验证产物后弃置。T21 补测（0673d6e shutdown draining）内容到位但 lib.rs 破 300 行（305）致 line-limit 全仓红，退回按 *_tests.rs 约定拆分；T21 账本维持 doing。T20 自报遗留「真实跨网 E2E」归 T27 与 T21 恰好重叠确认。T22 实现中。
- 2026-09-04 检查轮 57（RS 协调）：**T21 翻 done**（mergedMain=9fbe79a）——拆分（e4359cb，lib 200 行+tests.rs 103 行）与确定性门控排空测试（855f103）两轮小返回后终验通过；排空测试首轮在全量并行下暴露「未受理请求」接受竞态，按契约口径判测试过宽而非 serve 缺陷，返回改门控方案。全量门禁两次被外来件打断（fmt：主树 cli 会话在途 static_peers.rs 未格式化；gui-check：diagnostics 轨 b84eb10 用 t() 未声明 hook TS2304），前者由归属会话自行补修（1747d81），后者归属会话已收工清场，按 8910de0 代修先例协调者在隔离 worktree 修复直推 main（9fbe79a）。**批次一仅剩 T22**。T25 playbook 提前派单（零依赖，专属新会话 89f85ec3），批次二 T23 待 T22 合并即派。门禁判定学：主树被用户侧/他轨会话高频占用时，验收一律开隔离 worktree 钉 origin/main 跑，摆脱在途搅动。
- 2026-09-04 检查轮 58（RS 协调）：**T22 翻 done，批次一 3/3 关闭**（35ad480，13 文件 1929 行一次通过：67 tests + 隔离全量门禁 exit 0；模块划分 risk/redline 数据+旁路/scope/approval/whitelist 与计划书逐条对齐）。**批次二开闸**：T25 playbook（零依赖，89f85ec3）与 T23 只读工具面（依赖已齐，新专属会话 898caa4e，任务书含监狱逃逸矩阵/截断断言/执法接线/审计钩子接缝）并行 doing；T24 待 T25 命令清单接口、T26 待 T23。剩余队列 T24∥T26 → T27 → T28。
- 2026-09-04 检查轮 59（RS 协调）：**T23 翻 done**（mergedMain=5c9b9b8，一次通过）——两笔提交（执法接线+审计接缝/guarded host+四工具装配），34 tests 完整命中任务书矩阵（监狱逃逸含 symlink/dotdot/absolute、截断字符边界、执法原因链、审计事件、宿主全链），隔离全量门禁 exit 0；期间协调者误读其合并中间态为越界提交，取证（origin/feat/rs-tools-readonly tip 比对）后排除。**批次三开闸**：T26（p2p 接入+票据+审计落盘+session_report，专属新会话 801a58c9）派单；在途 T25（playbook，89f85ec3）+ T26 并行；T24 待 T25、T27 待 T26。
- 2026-09-04 检查轮 60（RS 协调）：**T25 翻 done**（mergedMain=b7b831f，一次通过）——解析器 9 模块 1263 行（lib.rs 292 贴线合规）+ 三类草案 379 行，31 tests 绿，shell_union 导出即 Q7 白名单数据源。**T24 立即派单**（3adc549c）：内嵌白名单数据 == shell_union 并集的一致性测试机械防漂移（enforce 运行时零 playbook 依赖，dev-dep 对账），管道/重定向特征一律闭集外拒。在途 T24 ∥ T26；此后 T27 → T28 收官。
## IM 聊天阶段（2026-09-04 派单，IM 协调 session-b7d42619，与 E10/RS 双轨并行）

定位：好友间 1:1 私聊（文本/emoji/图片/音频/视频/文件附件），离线可投递，不做实时通话/群聊。
契约出处：docs/design/im-chat-design.md（冻结基线）+ gui-contract.md §12（v7 加法）。底座（p2p-* 内核）只读；
全部实现落新建 crates/p2p-chat + apps/gui 消费面。范围互斥：与 E10（llm-share-*）、RS（repair-*）零交集，
根 Cargo.toml 仅三方各自追加，账本 coordinatorSession 不动、phase 记 IM-parallel。

| 任务单 | 负责会话 | 分支 | 范围 | 验收 |
|---|---|---|---|---|
| T29 p2p-chat 核心 crate | p2p-IM-T29（专属新会话） | feat/im-chat-core | 新建 crates/p2p-chat/**（协议帧/模型/好友簿/存储/outbox/发送接收）+ crates/p2p-chat/tests/ + wire-protocol.md §8 登记；根 Cargo.toml 仅 workspace.members 追加；p2p-* 内核只读 | cargo test -p p2p-chat + cargo clippy -p p2p-chat -- -D warnings + make check 全绿；itest 双节点回环（文本+附件+离线 flush+ACK）；wire-protocol.md §8 已登记 |
| T30 GUI chat 契约面（ipc/route 壳） | p2p-IM-T30（专属新会话） | feat/im-gui-shell | apps/gui/src：ipc-types/ipc/mock 的 chat 段、路由 /chat 壳页、menu.def.ts 追加、i18n 中英 chat 词条、chat 空视图组件 | cd apps/gui && pnpm build + pnpm test --run + pnpm check:i18n 全绿 + make check 全绿 |
| T31 聊天页完整交互（依赖 T30） | 待派（T30 合入后） | feat/im-chat-ui | apps/gui/src/views/chat/ + components/chat/ + stores/chat-store.ts（会话列表/气泡/输入条/表情/附件/预览/文件打开，mock 驱动） | pnpm build + pnpm test --run 全绿；交互组件测试（发文本/选表情/选附件/状态渲染） |
| T32 Tauri chat 接线（依赖 T29） | 待派（T29 合入后） | feat/im-tauri-wiring | apps/gui/src-tauri/**：装配 p2p-chat、chat_* 命令注册与 chat_message/chat_status 事件、dataDir/chat 接线、assetProtocol scope 含 chat/media、契约 roundtrip 测试 | cd apps/gui/src-tauri && cargo test + cargo clippy -- -D warnings + cd ../.. && make check 全绿；双命令冒烟（friends add/list） |
| T33 全链 E2E + 演练文档（依赖 T29+T32） | 待派（T29+T32 合入后） | feat/im-e2e | crates/p2p-itest/tests/chat_e2e.rs + docs/ops/im-chat-drill.md | cargo test -p p2p-itest --test chat_e2e + make check 全绿；两节点全链（加好友→文本→附件→重启 flush→历史回读）；演练清单在位 |

- 2026-09-04 检查轮 61（IM 协调 session-b7d42619，用户指令「加一个 im 聊天系统」接任阶段负责人）：设计定稿
  docs/design/im-chat-design.md + gui-contract.md §12（v7 契约冻结）直接落 main；主树基线 make check exit 0
  （GATE=0）确认后开闸派单。批次一 T29 ∥ T30 并行（范围互斥：新 crate vs apps/gui/src，契约 v7 双端对编程互不等）；
  T31 待 T30、T32 待 T29、T33 待 T29+T32。任务书只含需求与机械验收、不含源码。与 E10/RS 双轨互斥确认（无文件交集）。
- 2026-09-04 检查轮 62（RS 协调 session-5e733ff8，顺延 IM 轨占用的 61 号）：**T25 翻 done**（mergedMain=b7b831f，一次通过 31 tests + 隔离全量门禁 exit 0）——完工报告补记：三类草案命令并集经 shell_union 导出为 T24 数据源（Q7 闭环）。事故自纠存档：T25 会话 worktree 内 gui-check 软链主树 node_modules 致主树 apps/gui/node_modules 损坏，其自用 pnpm install --frozen-lockfile 重建并复跑 gui-check PASS 自愈；教训已由其沉淀——worktree 内 gui-check 禁软链主树 node_modules。T24 补充输入已流转（Disable-ScheduledTask/Get-Package 卸载类命令 argv0/参数边界需再核对）。RS 在途：T24 ∥ T26，此后 T27 → T28。
- 2026-09-04 检查轮 63（RS 协调）：**T26 翻 done**（mergedMain=0e0222d，一次通过）——四笔提交链（票据矩阵 7c47bc5/JSONL 后端 df4db1d/session_report 1896372/p2p 端点受理 0e0222d），helper 77 tests，LoopbackHub 复刻真实收流全链，隔离全量门禁 exit 0。**任务表缺口补充立卡 T23b**：shell_exec 进程执行面原无归属（T24 仅覆盖 enforce 判定语义），派 T23 会话（898caa4e，自荐待命）承接，branch feat/rs-shell-exec；**T27 提前派**（依赖 T20+T26 已齐，新专属会话 cfcb15a4）：桥⇄助手全链 E2E，时序纪律在册。在途 T24 ∥ T23b ∥ T27；T28 待 T27。第三轨备注：IM 聊天轨（IM 协调 b7d42619）占 coordination 变更记录 61 号，RS 自 62 号顺延。


