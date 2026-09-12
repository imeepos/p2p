# 任务书 TC：p2pctl tunnel connect（headless 访侧）+ 双节点 itest

类型：code-quality | 预算：120 次调用，分段检查点回报（40/80/120）|
分支：feat/wtc-cli-connect | worktree：.worktrees/wtc-connect（自建）
派发者显式 id：session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8
前置：TB（feat/wtb-proxy-sink）已合并 main——本卡 itest 消费 local_proxy 真实现。

## 目标（一句话）

新增 `p2pctl tunnel connect`：headless 访侧本地回环反代（对称 serve 形态），
让无 GUI 节点经 p2p 使用对端任意白名单内 `127.0.0.1:<port>` 的 HTTP/WS 服务；
双节点真链路 itest 覆盖 HTTP GET 与 WebSocket 双向。

## 背景

被访侧已有 `p2pctl tunnel serve`（apps/cli/src/tunnel/serve.rs，W-T5）：前台
常驻、SIGINT/SIGTERM 优雅收口、stdout JSON 行 ready/stopped。访侧此前只有
GUI 形态。TB 卡正把反代核心下沉 crates/p2p-tunnel::local_proxy（LocalProxy/
TunnelOpener 冻结面，签名表 .orchestrator/2026-09-12-tunnel-any/ta-PROGRESS.md）。
gui-contract §19.3 约束 8 已为 connect 预留对等 exempt 条款。

先读：AGENTS.md → 本任务书 → apps/cli/src/tunnel/serve.rs（形态模板，逐条对齐）
→ ta-PROGRESS.md 冻结导出面 → crates/p2p-tunnel/src/local_proxy/ 桩面
→ apps/gui/src-tauri/src/tunnel/visit.rs 的 NodeTunnelOpener（opener 装配先例）。

## 产出物

1. apps/cli/src/tunnel/connect.rs：
   - 参数：`--peer <PEER_ID>`（base58，必填）、`--target 127.0.0.1:<port>`
     （必填，字面量校验先校验后动作，对齐 serve 的 build_config 风格）、
     data_dir/quic_port/tcp_port/no_mdns/bootstrap 与 serve 逐项同名同默认。
     **无 --listen 参数**（裁定：LocalProxy 只用 bind(127.0.0.1:0)，确定性
     端口属后续契约加法候选，backlog 已记）。
   - 运行形态逐条对齐 serve.rs：前台常驻；SIGINT/SIGTERM 优雅收口（关监听→
     在途排空→终态 JSON 行；排空超时显式非零退出禁静默）；stdout 只发 JSON 行：
     `{"kind":"ready","localAddr",...,"target","peerId","maxConcurrent"可无}`、
     收口行 `{"kind":"stopped",...}`（字段风格对齐 serve 的 ready/stopped）。
   - 访侧 opener：connect.rs 内小结构体（p2p::Node + TunnelClient 装配，
     impl p2p_tunnel::TunnelOpener，参照 visit.rs NodeTunnelOpener 形状 ~25 行）。
   - 本地反代零自研：crate LocalProxy::bind(target_port, peer_id, opener)。
2. 单测（connect.rs 内 tests 模块，对齐 serve.rs 测试风格）：参数校验
   （target 非 127.0.0.1 字面量/端口 0/peer 非 base58 整体拒绝）等可纯测面。
3. itest：crates/p2p-itest/tests/tunnel_connect_wave.rs——双节点真链路：
   - 被访节点 A：TunnelGate allowlist 挂两个真实回环服务端口（测试内 spawn：
     一个 HTTP 服务返回固定 body、一个 WS echo 服务；用 workspace 既有 HTTP/
     WS 依赖，读 crates/p2p-itest 现有依赖自取，禁新增外部依赖）。
   - 访侧节点 B：直连调用 connect 运行时（非子进程），拿到 localAddr。
   - 断言：经 B 本地反代 HTTP GET 得预期 body；WS 升级成功 + 双向 echo 帧往返。
   - 复用 tunnel_common 裸流工厂（装配 MUST：工厂禁包 new_stream，协议 ID
     恰一帧，specs/tunnel.md §2.1）。
4. 登记面：docs/ops/p2pctl-ai-guide.md tunnel 域补 connect 条目 +
   `make` 内 ai-docs-sync 口径绿（新 CLI 命令必须登记）。cli-parity 本卡不动
   （无 GUI 对应命令；exempt 行由 TD 卡补）。
5. .orchestrator/2026-09-12-tunnel-any/tc-PROGRESS.md：分段进度 + 复盘移交。

## 验收标准（可机械核验，逐条带退出码）

- apps/cli 包聚焦测试 EXIT=0（包名读 apps/cli/Cargo.toml 自取）。
- `cargo test -p p2p-itest --test tunnel_connect_wave` EXIT=0（HTTP+WS 断言全绿）。
- cargo clippy 触及 crate（apps/cli 所在包/p2p-itest）--all-targets -D warnings EXIT=0。
- cargo fmt --check EXIT=0。
- ai-docs-sync EXIT=0（ai-guide 已登记）。
- bash scripts/check/tests/panic-hygiene.sh EXIT=0（2026-09-12 TB 卡教训：
  make 级门禁必须在自验清单内，禁只跑 cargo 面；本卡若在非测试路径写
  unwrap/expect/panic 即红）。
- git diff main --stat 全部落在 apps/cli/ + crates/p2p-itest/ +
  docs/ops/p2pctl-ai-guide.md + .orchestrator/2026-09-12-tunnel-any/。

## 边界（明确不做）

不改 apps/gui、crates/p2p-tunnel、gui-contract/specs（发现契约问题 BLOCKED
回报）；不做 --listen/--socks 等形态扩展；不改 serve 既有面；cli-parity.tsv
零改动。

## 已知坑（仓内踩坑记录）

- TunnelClient::open 内部带 pump（wire=帧面），测试对端读法必须匹配帧界，
  裸 read 会把帧头当数据（KI 记录在案）；直连/泵切换时对端读法跟着换。
- spawn 参数表达式 spawn 前求值：await 不能进 spawn 参数，否则互等死锁
  （KI:609 原案）。
- 泵测试必须并发对拷，先写后读在缓冲 <1MiB 时自锁（W-T2 复盘）。
- 验证命令禁管道收尾吃退出码；提交 git commit -F -；门禁跑批期间禁改同 worktree。

## 汇报与停止

DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT + 行为证据（命令+真实
退出码）+ 结构化复盘（做对了/踩坑/给 TE 的移交——TE 是跨机 e2e 证据卡）。
完成 push origin 分支，禁自合并，终报后冻结不再追加推送。停止条件：发现
LocalProxy 冻结面无法支撑 connect 形态（缺导出/语义不符）、或 WS 用例在
yamux 线上出现无法归因的半关/缓冲死锁。
