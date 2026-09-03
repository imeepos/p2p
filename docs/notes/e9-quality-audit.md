# E9 代码质量审计报告（E9-Q0）

- 基线：main @ abbb254（2026-09-03 审计快照，自建 worktree docs/e9-quality-audit）。只读审计：未修改任何 crates/ apps/ scripts/ Makefile 代码文件，未触碰 docs/coordination.md、.devloop/、.agents/ 与 .worktrees/e8-* 三个在途 worktree。
- 方法：wc 与脚本扫描（函数行数按大括号配对近似，top 项经人工精读核对）+ cargo tree 取证 + grep 抽样 + 分文件精读。
- 范围：crates/ 12 crate 约 18.9k 行 Rust；apps/gui/src 约 12.9k 行 TS/TSX。

## 一、结构红线

### 1.1 超 300 行文件清单

无。门禁 scripts/check/line-limit.sh:2 默认 LINE_LIMIT=300（超限 exit 1），当前全绿。

### 1.2 恰好 300 行贴线（实测核对确认）

| 文件 | 行数 | 构成 |
|---|---|---|
| crates/p2p-swarm/src/swarm/mod.rs | 300 | 产线 300（测试已外置 swarm/tests.rs 与 book_tests.rs，声明见 :33-36） |
| crates/p2p-security/src/noise.rs | 300 | 产线 248 + 内联测试 52（:248-300） |

拆分分析（应否拆、按什么职责拆）：

- swarm/mod.rs：已是纯装配+API 门面（14 个子模块声明 :18-36），300 行几乎全为 impl Swarm 约 20 个方法，零余量；E8 正在并行改 swarm，任何追加必破线。应按职责拆两簇，均为纯移动零行为变更：
  1. 地址簿面：add_peer_addresses(:180)、add_peer_addresses_with_source(:186)、on_peer_expired(:211)、set_observed_addrs(:235)、addresses_of(:241)，约 70 行，移入 book.rs 追加 impl Swarm 块（Rust 同 crate 跨文件 impl 合法，pool.rs 已是先例）。
  2. 中继会话命令面：relay_degrade(:265)、has_relay_sessions(:293)、add_relay_session(:297)，约 36 行，移入 relay_session.rs。
  拆后 mod.rs 约 220 行，回到推荐区间。结论：应拆，两簇移动即可，无需引入新抽象。
- noise.rs：产线 248 行本身健康（outbound(:55) / inbound(:86) 各约 30 行单职责）。超线压力全部来自内联测试 52 行（:248-300）。克制方案：仅把 mod tests 外移到 noise/tests.rs（仿 swarm/mod.rs:33-36 的 book_tests 先例）即回落 248，本轮足够；可选进阶（非必需）：身份绑定簇 x25519_private(:129) / x25519_public(:142) / make_payload(:164) / verify_payload(:175) 共约 76 行按「身份绑定」职责下沉 noise/identity_payload.rs。
- 次贴线 src：crates/p2p-relay/src/slots.rs 294（内联测试 87 行 :209-294，外移即 207；逻辑高内聚，本轮不必拆逻辑）。

### 1.3 超 60 行函数清单（非测试代码，大括号配对实测）

| 行数 | 位置 | 函数 |
|---|---|---|
| 84 | crates/p2p-swarm/src/swarm/relay_session.rs:175-258 | degrade |
| 68 | crates/p2p-cli/src/bootstrap.rs:64-131 | run |
| 67 | crates/p2p-swarm/src/swarm/responder.rs:29-95 | handle_punch_req |
| 65 | crates/p2p-swarm/src/swarm/relay_session.rs:91-155 | session_loop |
| 65 | crates/p2p-swarm/src/swarm/dial.rs:33-97 | dial_peer |
| 62 | crates/p2p-relay/src/circuit.rs:47-108 | bridge |

- degrade（84 行）唯一显著超标：单函数串联 5 阶段——电路预留（:176-182）、信令有界重试环（:188-207）、直连探测环（:216-237）、打洞成功上报（:224-232）、中继兜底接入（:245-256），可按阶段拆 reserve_and_signal / probe_direct / join_circuit 三段。
- bootstrap.rs run（68 行）混合参数装配与信号循环；CLI 入口可接受，建议至少把 select 事件循环（:105-130）抽成独立函数。

### 1.4 40-59 行重点观察项（src 抽样 top）

59 crates/p2p-relay/src/slots.rs:105 on_connect；58 crates/p2p-transport/src/quic.rs:138 dial；58 crates/p2p-security/src/noise_stream.rs:74 advance_rx；57 crates/p2p/src/assembly.rs:27 build；56 crates/p2p-mux/src/yamux_mux.rs:87 drive；56 crates/p2p-discovery/src/mdns.rs:218 run。
均低于 60 红线且单一职责，本轮不动，列作回归观察项。

### 1.5 距 300 红线最近的 top10（src 非测试文件）

300 crates/p2p-swarm/src/swarm/mod.rs；300 crates/p2p-security/src/noise.rs；294 crates/p2p-relay/src/slots.rs；282 crates/p2p-swarm/src/swarm/relay_session.rs；280 crates/p2p-discovery/src/mdns.rs；277 crates/p2p-swarm/src/pool.rs（产线 149+测试 128）；267 crates/p2p-relay/src/limits.rs；266 crates/p2p-relay/src/client.rs；265 crates/p2p-protocol/src/lib.rs（产线 223）；264 crates/p2p-cli/src/cli.rs。
（测试文件贴近线：crates/p2p-relay/tests/relay_stability.rs 298、crates/p2p-itest/tests/peer_lifecycle.rs 298、crates/p2p/tests/facade.rs 278、crates/p2p/tests/m3_chain.rs 267。）

## 二、模块边界

### 2.1 crate 依赖方向（cargo tree --workspace -e normal --depth 1 与各 Cargo.toml 双重取证）

| crate | 内部依赖（normal） |
|---|---|
| p2p-identity / p2p-log / p2p-mux | 无（base 层） |
| p2p-security | identity, mux |
| p2p-transport | identity, mux, security |
| p2p-discovery | identity, transport |
| p2p-protocol | identity, mux |
| p2p-relay | mux（单依赖） |
| p2p-swarm | identity, mux, protocol, relay, security, transport |
| p2p（facade） | discovery, identity, mux, protocol, swarm, transport |
| p2p-cli | p2p, identity, log, mux, protocol, relay, transport |
| p2p-itest | discovery, relay, swarm, identity, mux, protocol, security, transport |

分层实证：base(identity/log/mux) -> security/transport/discovery/protocol/relay -> swarm -> facade -> cli/itest。
无环验证：cargo tree -i p2p-transport -e normal 与 -i p2p-security -e normal 反向核对无回路；Cargo.toml 中疑似反向条目（mux 到 transport 等）均为 [dev-dependencies]，normal 图中 p2p-mux 零内部依赖（cargo tree -p p2p-mux -e normal 实证）。security 到 mux 方向合理：mux 定义全仓流抽象 BoxedStream（security/lib.rs:8、noise.rs:17 消费），不构成越层。

### 2.2 越层 / 绕过 facade 事实

- cli 绕过 facade 自建 relay 服务端装配：crates/p2p-cli/src/bootstrap.rs:15 引 p2p_relay，:141-164 spawn_relay 自绑 quic/tcp 并自转 accept 循环。facade（p2p）依赖表无 p2p-relay/p2p-security，「经 facade 起带 relay 服务端的节点」在 facade 层不可能。建议 E9+ 以加法入口评估 facade relay 角色装配；当前先登记事实。
- swarm 借 relay crate 转发协议常量：crates/p2p-swarm/src/swarm/ping.rs:23 pub const PING_PROTOCOL: &str = p2p_relay::proto_ids::PING（:22 注释自认「同源」）。ping 协议 ID 概念上属协议层（p2p-protocol 更合适），现居 relay crate。低危，登记不改。

### 2.3 pub 面泄漏抽样（应为 pub(crate) 或不导出）

- crates/p2p-swarm/src/lib.rs:19 pub use pool::ConnectionPool：全仓 grep 无 swarm 外使用点（仅 pool.rs 自身与测试），应降 pub(crate)。
- crates/p2p-relay/src/lib.rs:56 pub use service::RelayServiceImpl：具体实现类型泄漏为 API。外部共 5 处直接构造：crates/p2p-cli/src/bootstrap.rs:148、crates/p2p-itest/src/lib.rs:78、crates/p2p-itest/tests/relay_control_resilience.rs:25、crates/p2p-itest/tests/relay_transport_persistence.rs:49、crates/p2p/tests/m3_chain.rs:48。外部真正需要的只是 lib.rs:18 的 trait RelayService（Arc<dyn RelayService>）。
- crates/p2p-relay/src/lib.rs:50 pub use link::{ mock_link_pair, MockLink, MockLinkSource, ... }：测试替身挂在产线 API 上，被 crates/p2p-itest/src/lib.rs:72-74 与 crates/p2p/tests/m3_chain.rs:48 消费。应收进 #[cfg(any(test, feature = "test-util"))] 或迁往 p2p-itest（该 crate 职责即测试基建）。
- 正面对照：crates/p2p-swarm/src/lib.rs:15-22 re-export 面克制；identity/mux/protocol 导出面干净。

### 2.4 跨 crate 重复代码

- RelayServiceImpl 装配样板 5 处（见 2.3 构造点清单）：各处自备 source/serve spawn/limits，可收敛为 relay 提供工厂（如 RelayService::spawn(source, limits) 返回 Arc<dyn RelayService>），消 4 处重复。
- PING_PROTOCOL 双名（见 2.2）。高频 fn new/default/fmt 属惯用法；poll_* 多处实现是各 AsyncWrite 适配器自身职责。未发现成段复制粘贴。

### 2.5 12 crate 划分评价

结论：划分健康，本轮不建议合并或再拆。依据：依赖图五层清晰且无环（2.1）；每 crate 有独立变更理由——p2p-log 708 行小但 panic 钩子+滚动日志被 cli 独占消费；p2p-identity 269 行是全仓信任根；p2p-mux 590 行定义 BoxedStream 被 5 个 crate 依赖，并入 transport 会强迫 protocol/security 拖上传输实现；p2p-relay 仅依赖 mux 是全仓最干净的边界样板。唯一候选议题是 2.2 的 facade-relay 角色缺位，属能力补齐而非边界错误，克制起见仅登记。

## 三、可读性

### 3.1 分 crate 抽样（每 crate 1-2 个核心文件；命名/注释/错误处理/magic number）

| crate | 抽样 | 评价（文件:行号证据） |
|---|---|---|
| p2p-identity | lib.rs:1-56 | 命名好；注释低废（design §6 引用）；无错误路径；无 magic |
| p2p-mux | yamux_mux.rs | 错误链保真注释到位（:164-165「拒绝 to_string 拍平」），ChainedPayload 实现（:167-173） |
| p2p-protocol | lib.rs | thiserror 全枚举（:70-84）；varint 溢出显式拒绝带 why 注释（:194-200）；MAX_FRAME_SIZE 具名（:29） |
| p2p-security | noise.rs / noise_stream.rs | SecurityError 统一；不可达路径留 error! 信号（noise.rs:147）；帧长 96/32/64 有构成注释（noise.rs:163,176,205） |
| p2p-transport | quic.rs / tcp.rs | 错误映射保 kind（yamux_mux.rs:167-173）；常量具名 |
| p2p-discovery | mdns.rs / rendezvous/client.rs | 退避参数全具名（client.rs:20-24、mdns.rs:20）；残留静默：rendezvous/link.rs:78 let _ = framed.close()（关闭路径，建议补 debug!）；mdns.rs:204 事件 send 忽略属无订阅者语义可接受 |
| p2p-relay | slots.rs / circuit.rs | TTL 具名（slots.rs:11-13）；但 slots.rs:69 以裸 u32 errcode 当错误类型，靠 errcode:: 常量防 magic（:71-76），类型化欠佳 |
| p2p-swarm | swarm/mod.rs / pool.rs / relay_session.rs | 错误处理不一致（详 3.2）；命名与注释好（E3/E4/E6 复盘编号引用） |
| p2p（facade） | assembly.rs | 失败路径留信号正面样板：观测全失败 warn（:43-48）、注册集无路由地址 warn（:72-78）；常量具名（:22-25） |
| p2p-cli | bootstrap.rs / metrics_log.rs | metrics_log.rs:8-24 .ok() 链静默降级默认值（env 解析，无日志）；relay serve 失败有 error!（bootstrap.rs:150-154）；+3 端口偏移 magic（:83-85）；裸 10ms 轮询（:181,208） |
| p2p-log | rolling.rs / lib.rs | init_once 43 行聚合清晰；单测充分 |
| p2p-itest | lib.rs | expect_within 有界等待防悬挂（:89-94）是测试基建正面样板 |

总体：命名一致性好；注释废话率低（几乎条条带 design § 或事故编号，未发现套话）；非测试路径无 unwrap/expect（Makefile:44-45 panic-hygiene 门禁兜底）。

### 3.2 错误处理一致性（静默吞错残留）

- swarm 层 io::Error::other(e.to_string()) 拍平错误链共 10 处：crates/p2p-swarm/src/swarm/mod.rs:269,282,287,290、crates/p2p-swarm/src/swarm/relay_session.rs:167,181,202,210,214,247-248。与 mux 层 yamux_mux.rs:164-173 明文确立的 ChainedPayload 保真模式自相矛盾，E9 应统一（保留 source 可 downcast）。
- 其余静默点：crates/p2p-discovery/src/rendezvous/link.rs:78、:91（send 忽略属接收端已关可接受；close 建议留 debug!）；crates/p2p-cli/src/metrics_log.rs:8-24（降级无日志）。
- crates/p2p-cli/src/bootstrap.rs:150-154 relay serve 后台任务失败仅 error! 不上抛，进程存活但 relay 半死；可观测信号已有，登记即可。

### 3.3 magic number 残留清单

crates/p2p-cli/src/bootstrap.rs:84-85 中继端口 +3 偏移（有注释无具名，建议常量化）；同文件 :181,208 from_millis(10) 轮询间隔；crates/p2p-itest/tests/peer_lifecycle.rs:247 +100ms 余量。其余时长扫描均具名常量（见 3.1 表）。

### 3.4 apps/gui 前端（12,931 行 TS/TSX）

- zustand store 结构（stores/ 合计 766 行）：3 store 分域清晰——node-store.ts（节点状态+动作，132 行）、update-store.ts（107 行）、event-reducer.ts（105 行纯函数 reducer，node-store.ts:15 消费）。selector 做引用稳定缓存防 React 快照不收敛（node-store.ts:91-119，附白屏事故注释），质量好。
- 结构问题抽样：
  1. mock 后端无条件进产物：apps/gui/src/lib/ipc.ts:4-5 顶层 import mock-ipc（342 行）与 mock-diagnostics，仅 ipc.ts:22 VITE_MOCK_IPC === "1" 决定运行时选择，两套 mock 全量进 prod bundle；应改动态 import 隔离。
  2. 死代码：apps/gui/src/components/feedback/feedback-demo-card.tsx:17-114（98 行组件）全仓零引用（grep 仅自身文件命中）。
  3. 模块级单例 subscriptionStarted（node-store.ts:36-47）绕过 store 状态，HMR 下可能假已订阅；订阅 unlisten 被丢弃 void unlisten（node-store.ts:52），应用生命周期订阅可接受但缺契约注释。
- 组件层级健康：App.tsx 29 行；routes/*.tsx 各 1-2 行薄壳；views 按域分组（monitor/discovery/diagnostics/settings/relay/update/shared）。最大自研组件 views/monitor/peers-table-card.tsx 204 行，均低于红线。components/ui/ 的 dropdown-menu.tsx 255、select.tsx 186 为 shadcn 生成物，line-limit 门禁只查 crates/ 不冲突，无需处理。
- i18n：locales/en-US.ts 451、zh-CN.ts 439，append-only 登记文件，暂不拆；体积翻倍时按命名空间分文件。

## 四、测试质量

### 4.1 测试代码占比（内联 #[cfg(test)] 块脚本统计；crate 总行含 tests/ 目录）

| crate | 总行 | 内联测试 | 占比 | tests/ 目录 |
|---|---|---|---|---|
| p2p-itest | 2054 | 0 | 0%（整 crate 即测试基建） | 2054 |
| p2p-relay | 3344 | 438 | 13% | 546 |
| p2p-swarm | 3604 | 359 | 9% | 已内嵌子文件（swarm/tests.rs 270 等） |
| p2p（facade） | 1858 | 105 | 5% | 881 |
| p2p-cli | 1271 | 275 | 21% | 120 |
| p2p-log | 708 | 179 | 25% | 0 |
| p2p-protocol | 813 | 120 | 14% | 238 |
| p2p-security | 941 | 76 | 8% | 92 |
| p2p-mux | 590 | 53 | 8% | 76+（idle_reopen、error_chain） |
| p2p-discovery | 2538 | 179 | 7% | 65+（另有 src 内嵌 tests.rs 已计入内联外文件） |
| p2p-transport | 920 | 47 | 5% | 406+ |
| p2p-identity | 269 | 21 | 7% | 79 |

全仓约 6.6k 行测试 / 18.9k 行 Rust 约 35%。偏薄：facade 5%、transport 5%、discovery 7%。

### 4.2 itest 接缝清单与缺口

已覆盖（crates/p2p-itest/tests/ 12 文件）：peer_lifecycle（状态机/退避重连）、protocol_stack（安全栈 2MiB 分块）、connection_liveness、security_identity（篡改帧双向失败）、rendezvous_e2e、relay 三件套（relay_messages_limits / relay_control_resilience / relay_transport_persistence）、metrics_dialhop 与 dialhop_observability、hairpin_fastfail、tcp_wan_bootstrap。
缺口：
- mdns 无 itest 接缝测试：仅 crates/p2p/tests/facade_mdns.rs:59 带 ignore 原因标注，缺注入式替代。
- p2p-log 零 itest 接缝（滚动/panic 钩子仅 crate 内单测，rolling.rs:151-205）。
- relay 服务端端到端接缝测试放在 facade crate 的 crates/p2p/tests/m3_chain.rs:17-48，未进 itest，位置漂移。
- keepalive/保活时长类接缝仅 relay crate 内 relay_stability.rs 覆盖（该文件留给 E9 长稳复测单）。

### 4.3 依赖环境/时序的脆弱测试清单

- crates/p2p-mux/tests/idle_reopen.rs:24 sleep(2s) 真实时钟等待，拖慢门禁且 CI 易抖。
- crates/p2p-relay/tests/relay_stability.rs:174,222,240,288（80ms/600ms/1s/5ms 真实 sleep）——归 E9 长稳复测单处理，此处仅登记。
- crates/p2p-itest/tests/peer_lifecycle.rs:247 RESET_MIN_UPTIME + 100ms 真实 uptime 门槛。
- crates/p2p-discovery/src/cache.rs:108,119,133 thread::sleep(30ms)（内联测试真实时钟）。
- crates/p2p-itest/tests/tcp_wan_bootstrap.rs:58 JITTER sleep。
- 全仓无 tokio::time::pause/start_paused 使用（grep 零命中）：上述 tokio 测试多数可改虚拟时钟消除时序脆弱。
- 正面：环境依赖测试全部 #[ignore] 且带原因（crates/p2p-discovery/tests/mdns_live.rs:24、crates/p2p/tests/facade_mdns.rs:59）；itest 有界等待原语 expect_within（crates/p2p-itest/src/lib.rs:89-94）。

## 五、E9 建议：修复轮任务单草案（5 张）

范围互斥可并行（唯 T1 到 T3 因 relay_session.rs 交叉显式串行）；均不依赖 E8 metrics，与「长稳复测+保活间隔自适应」单零文件交集（relay_stability.rs 已让给该单）。

### T1 swarm/mod.rs 瘦身拆分 [P1, S]

- 范围：crates/p2p-swarm/src/swarm/mod.rs、book.rs、relay_session.rs（纯移动，零行为变更）
- 问题：恰 300 行零余量（1.2），E8 并行追加必破线
- 动作：地址簿 API 簇移 book.rs、中继会话命令簇移 relay_session.rs（跨文件 impl Swarm）
- 验收：test $(wc -l < crates/p2p-swarm/src/swarm/mod.rs) -le 260 && cargo test -p p2p-swarm --quiet
- 规模：S（半天内）；优先级 P1

### T2 noise.rs 测试外移 [P1, S]

- 范围：crates/p2p-security/src/noise.rs（新增 noise/tests.rs）
- 问题：恰 300 行，52 行内联测试挤占（1.2）
- 动作：mod tests 外移 noise/tests.rs（仿 swarm/book_tests 先例）
- 验收：test $(wc -l < crates/p2p-security/src/noise.rs) -le 250 && cargo test -p p2p-security --quiet
- 规模：S（半小时级）；优先级 P1

### T3 swarm 错误链保真与长函数拆分 [P1, M]（须在 T1 合入后执行）

- 范围：crates/p2p-swarm/src/swarm/relay_session.rs、dial.rs、responder.rs
- 问题：io::Error::other(to_string) 拍平错误链与 mux ChainedPayload 模式冲突（3.2）；degrade 84 行 / handle_punch_req 67 行 / dial_peer 65 行超 60 红线（1.3）
- 动作：错误改 io::Error::new(kind, ChainedPayload) 或等价保 source；degrade 按 5 阶段拆 reserve_and_signal / probe_direct / join_circuit；handle_punch_req 与 dial_peer 同法拆
- 验收：! grep -rq "io::Error::other(e.to_string())" crates/p2p-swarm/src && test $(wc -l < crates/p2p-swarm/src/swarm/relay_session.rs) -le 300 && cargo test -p p2p-swarm --quiet
- 规模：M（1 天）；优先级 P1

### T4 gui mock 产物剥离与死组件清理 [P2, S]

- 范围：apps/gui/src/lib/ipc.ts、mock-ipc.ts、mock-diagnostics.ts、components/feedback/feedback-demo-card.tsx
- 问题：mock 后端无条件进 prod bundle（ipc.ts:4-5,22）；feedback-demo-card.tsx:17-114 零引用（3.4）
- 动作：mock 改 VITE_MOCK_IPC 开关下动态 import；删除死组件
- 验收：cd apps/gui && pnpm build && ! grep -rq "mockBackend" dist/assets && test ! -f src/components/feedback/feedback-demo-card.tsx && pnpm test
- 规模：S（半天内）；优先级 P2

### T5 relay pub 面收口与装配收敛 [P3, M]（建议 E8 relay 单合入后排程）

- 范围：crates/p2p-relay/src/lib.rs、link.rs、service.rs + 使用点（crates/p2p-cli/src/bootstrap.rs、crates/p2p-itest/src/lib.rs、crates/p2p-itest/tests/relay_control_resilience.rs、crates/p2p-itest/tests/relay_transport_persistence.rs、crates/p2p/tests/m3_chain.rs）+ crates/p2p-swarm/src/lib.rs（ConnectionPool 一行降级）
- 问题：RelayServiceImpl 与 Mock 替身泄漏为产线 API，装配样板 5 处（2.3/2.4）；ConnectionPool 零外部使用仍 pub（crates/p2p-swarm/src/lib.rs:19）
- 动作：mock 替身迁 p2p-itest 或 feature test-util 门控；提供工厂返回 Arc<dyn RelayService>；crates/p2p-swarm/src/lib.rs:19 改 pub(crate) 重导出
- 验收：! grep -q "mock_link_pair" crates/p2p-relay/src/lib.rs && ! grep -rq "RelayServiceImpl::new" crates/p2p-cli/src crates/p2p/tests crates/p2p-itest --include="*.rs" && cargo test --workspace --quiet
- 规模：M（1-1.5 天）；优先级 P3

---

审计执行说明：本报告基于只读检查产出，审计会话未修改任何 crates/ apps/ scripts/ Makefile 文件。
