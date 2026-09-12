# 任务书 TB：local_proxy 实现填埋 + GUI 切换消费（行为零变化）

类型：code-quality | 预算：120 次调用，分段检查点回报（40/80/120）|
分支：feat/wtb-proxy-sink | worktree：.worktrees/wtb-sink（自建，基于最新 origin/main，开工先 fetch 核对）
派发者显式 id：session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8（send_parent 解析异常时 session_link_send 定向投递）

## 目标（一句话）

把 crates/p2p-tunnel/src/local_proxy/ 签名桩的全部 todo!("W-TB") 填上实现
（机械迁移自 apps/gui/src-tauri/src/tunnel/），GUI 切换为消费 crate 导出，
全程行为零变化（全部既有测试平移后必须绿，任何测试差异=缺陷）。

## 背景

TA 卡已冻结签名与契约（主干 @ 1ef3a4d8 起）：桩在
crates/p2p-tunnel/src/local_proxy/{mod,head,pump}.rs，签名表与移交清单在
.orchestrator/2026-09-12-tunnel-any/ta-PROGRESS.md（真值源，逐字照办）。
现实现全部在 apps/gui/src-tauri/src/tunnel/{proxy.rs(272),head.rs(270),
pump.rs(94),proxy_tests.rs(321),visit.rs(263),audit.rs(141),types.rs(131),url.rs(116)}。
你的下游：TC（CLI connect）与 TD（GUI 通用入口）都消费你落地的 crate 面。

先读：AGENTS.md → 本任务书 → ta-PROGRESS.md → 上述源文件。

## 产出物

1. crates/p2p-tunnel/src/local_proxy/ 填实现：
   - proxy.rs/head.rs/pump.rs 实现体机械平移，**逐字优先于改写**（重构诱惑零容忍，
     本卡唯一目标是「换住址不变行为」）；审计承载按 ta-PROGRESS 移交清单③：
     crate 内 TunnelAudit/TunnelAuditRecord 承载，PumpAudit 由 crate 内会话审计
     结构实现；camelCase 映射留 GUI 装配层。
   - 删光 todo!("W-TB") 与临时 #[allow(dead_code)]（保留项必须逐个给理由注释）。
   - **行数红线**：mod.rs 会超 300 行（现桩 123 行 + proxy.rs 272 行实现体），
     必须拆子模块（如 conn.rs 连接处理路径），单文件 ≤300 行。
2. proxy_tests.rs（321 行，超行数红线）迁入 crate 并**拆分**：
   crates/p2p-tunnel/src/local_proxy/tests/ 或 tests.rs+子文件；用例一条不减，
   回报迁移前后用例数对照表。
3. GUI 切换（apps/gui/src-tauri/src/tunnel/）：
   - 删 proxy.rs/head.rs/pump.rs 本地实现；visit.rs 改消费
     p2p_tunnel::{LocalProxy, TunnelOpener, ...}（NodeTunnelOpener impl crate
     trait，签名逐字同源）。
   - tunnel.rs 薄包装命令层**保持现状**（已知坑：tauri 命令宏不跟随 use
     re-export，#[tauri::command] fn 禁改为三段路径）。
   - types.rs 的 sessions 组装 = TunnelAuditRecord→TunnelSessionAudit
     camelCase 映射（§19.2 形状逐字不变）。
   - GUI 侧 URL/token/DSH 语义（url.rs/types.rs token 面）零触碰。
4. .orchestrator/2026-09-12-tunnel-any/tb-PROGRESS.md：分段进度 + 用例数
   对照表 + 给 TD 的移交注记（GUI 侧新的导入面）。

## 验收标准（可机械核验，逐条带退出码）

- `grep -rn "todo!" crates/p2p-tunnel/src/local_proxy/` → 0 命中。
- `cargo test -p p2p-tunnel` EXIT=0，迁移用例数 ≥ 迁移前（对照表为证）。
- `cargo test -p p2p-console` EXIT=0（GUI tauri 既有面不红）。
- `cargo clippy -p p2p-tunnel --all-targets -- -D warnings` EXIT=0。
- `cargo clippy -p p2p-console --all-targets -- -D warnings` EXIT=0（src-tauri 硬性）。
- `cargo fmt --check` EXIT=0。
- `git diff main --stat` 全部落在 crates/p2p-tunnel/ + apps/gui/src-tauri/src/tunnel/
  + .orchestrator/2026-09-12-tunnel-any/，禁区零触碰。

## 边界（明确不做）

不改 apps/cli、crates/p2p-itest、apps/gui 前端 TS/React、gui-contract/specs
契约文本（发现契约问题立即 BLOCKED 回报，禁自行改义）；不改 pump 语义
（半关闭/字节计数/空闲护栏口径按现实现）；不做 tunnel_open 命令（TD 的事）。

## 已知坑（仓内踩坑记录）

- spawn 参数表达式在 spawn 前求值——await 不能进 spawn 参数（KI:609 同族，
  迁移测试时特别小心）。
- 反代测试并发拷贝：先写后读在缓冲 <1MiB 时自锁，测试须模拟真实并发对拷
  （W-T2 复盘原话）。
- 验证命令禁管道收尾吃退出码；提交用 git commit -F -；门禁跑批期间禁改同
  worktree。

## 汇报与停止

DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT + 行为证据（命令+真实
退出码）+ 结构化复盘（做对了/踩坑/给 TD 的移交）。完成 push origin 分支，
禁自合并。停止条件：平移中发现实现依赖 GUI 内部类型无法在 crate 表达、或
测试平移后出现无法归因的红（先双查自身再怀疑契约）。
