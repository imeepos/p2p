# PROGRESS — TB：local_proxy 实现填埋 + GUI 切换消费（行为零变化）

分支 feat/wtb-proxy-sink ｜ worktree .worktrees/wtb-sink ｜ 基于 origin/main @ ba45afd8
类型 code-quality ｜ 状态：DONE（全部门禁绿，push origin，禁自合并）

## 分段进度

1. [x] 读 AGENTS.md / 任务书 / ta-PROGRESS.md（真值源）/ GUI 8 文件 / crate 桩 3 文件
   / lib.rs re-export 面 / audit.rs / wire.rs TunnelErrorCode / specs §5.2 /
   gui-contract §19.2+§19.3；基线核对（fetch 后 local main == origin/main @ ba45afd8）
2. [x] 建树：`git worktree add .worktrees/wtb-sink -b feat/wtb-proxy-sink origin/main`
3. [x] crate 填埋：mod.rs（trait/ctx/proxy，153 行）+ conn.rs（连接路径，186 行）+
   head.rs（195）+ pump.rs（114）+ session.rs（SessionAudit/SessionLog 活账 +
   impl PumpAudit，161）；todo!("W-TB") 零命中、临时 #[allow(dead_code)] 零命中
4. [x] 测试迁移拆分：tests/{mod,http,ws}.rs（公共缝/HTTP 路径/WS 升级路径）+
   head_tests.rs；单文件全部 ≤300 行（最大 199：GUI types.rs）
5. [x] GUI 切换：删 proxy.rs/head.rs/pump.rs/proxy_tests.rs；visit.rs 消费
   `p2p_tunnel::{LocalProxy, ProxyCtx, TunnelOpener}`（NodeTunnelOpener impl crate
   trait，async_trait 签名逐字）；audit.rs 只留 open_url；types.rs 增
   TunnelAuditRecord→TunnelSessionAudit 八字段映射；tunnel.rs 薄包装零触碰
6. [x] 门禁（逐条退出码见下）
7. [x] push origin feat/wtb-proxy-sink（禁自合并，合并归协调者）

## 验收门禁证据（真实退出码）

| 命令 | 结果 |
|---|---|
| `grep -rn "todo!" crates/p2p-tunnel/src/local_proxy/` | 0 命中（exit 1） |
| `grep -rn "allow(dead_code)" crates/p2p-tunnel/src/local_proxy/` | 0 命中（exit 1） |
| `cargo check -p p2p-tunnel --all-targets` | EXIT=0 |
| `cargo test -p p2p-tunnel` | EXIT=0，30 passed（17 存量 + 13 迁入） |
| `cargo test -p p2p-console`（src-tauri 自有 workspace） | EXIT=0（全 suite 绿，含 139 unit） |
| `cargo clippy -p p2p-tunnel --all-targets -- -D warnings` | EXIT=0 |
| `cargo clippy -p p2p-console --all-targets -- -D warnings` | EXIT=0 |
| `cargo fmt --check`（根 workspace + src-tauri） | 双双 EXIT=0 |
| `git diff main --stat` | crates/p2p-tunnel/ + apps/gui/src-tauri/src/tunnel/ + 双 Cargo.lock（见漂移④） |

## 用例数对照表（迁移前后）

| 来源（GUI 迁移前） | 用例数 | 去向（crate/GUI 迁移后） | 用例数 | 判定 |
|---|---|---|---|---|
| tunnel/proxy_tests.rs | 4 | local_proxy/tests/{http,ws}.rs | 4 | 一条不减 |
| tunnel/head.rs 内联 tests | 7 | local_proxy/head_tests.rs | 7 | 一条不减 |
| tunnel/audit.rs 内联 tests | 2 | local_proxy/session.rs tests | 2 | 一条不减（断言随 ended_at 哨兵换型） |
| tunnel/pump.rs | 0 | local_proxy/pump.rs | 0 | 原 GUI 即无 pump 用例 |
| tunnel/types.rs tests | 3 | tunnel/types.rs tests | 3 + 1 新增映射用例 | 净增 +1 |
| tunnel/url.rs tests | 2 | tunnel/url.rs tests | 2 | 零触碰 |
| 合计 | 13 迁移面 | | 13 + 1 | ≥ 迁移前 ✓ |

注：p2p-tunnel 存量 17 用例（responder/wire/config/client）不受影响；30 passed = 17 + 13。

## 漂移与决策登记（TA 桩注记的实现期落定，需 TA/TD 知悉）

1. **审计 "open" 编码**：`TunnelAuditOutcome` 不加 open 变体——apps/cli/src/tunnel/
   serve.rs:133 对其穷尽匹配（无 wildcard），加变体会编译打红禁区文件。改用
   `ended_at == 0` 哨兵承载 specs §5.2「ended_at 在会话存续期为空」；GUI 映射侧
   （types.rs From impl）以 `ended_at == 0` 判 `endedAt: null` + `outcome: "open"`。
   存续期记录的 outcome 字段为占位 Served（无消费方读它）。§19.2 形状与原 GUI
   行为逐字不变（open 用例仍先见 "open"/null，终态见 "ok"/错误码）。
2. **ProxyCtx 私有字段换型**：TA 桩草 `audit: TunnelAudit` →
   `local_proxy::session::SessionLog`（Arc<SessionAudit> 活账：存续期字节计数可见、
   finish 首写生效——GUI AuditLog 平移体）。快照元素仍是 crate `TunnelAuditRecord`
   （§5.2 八字段闭集），pub 签名面零变化。crate 内 TunnelAudit 语义（被访侧终态
   账）未动。
3. **new_uid 随机源**：p2p-tunnel Cargo.toml 加 `getrandom = "0.2"`（与 GUI 同版本
   串），实现体逐字平移；uid 语义 = 16 hex（§5.2）不变（TA 移交清单④授权 crate
   内替代）。
4. **Cargo.lock 连带**：双 lock（根 + apps/gui/src-tauri/Cargo.lock）各 +1 行
   （getrandom 解析项），系 3 的机械后果；未提交则两棵树均不可复现构建。lock 文件
   属构建产物登记，非源码禁区越界，特此报备。
5. **session_outcome 映射**：GUI finish 口径（"ok" | TunnelErrorCode 字面量）→
   crate 终态：非 "ok" 一律 `Broken(code)`（访侧记录方语义：未干净收口即故障）；
   GUI 反向映射只取 `code.as_str()` 裸码，Rejected/Broken 区分对 GUI 不可见，
   行为零变化。未知字面量兜底 Broken(Io)（产出为闭集，不可达不静默）。

## 给 TD 的移交注记（GUI 侧新导入面）

- `use p2p_tunnel::{LocalProxy, ProxyCtx, TunnelOpener}`（visit.rs 现状）；
  lib.rs re-export 面未动：`p2p_tunnel::{Head, HEAD_MAX, read_head, PumpAudit,
  TUNNEL_IDLE_GRACE, read_reply_head, is_101, forward_exact, drain_reply}` 同源可用。
- `NodeTunnelOpener` 保持 `#[async_trait] impl TunnelOpener`（crate trait，签名逐字）。
- sessions 组装：`ctx.audit_snapshot().await.into_iter().map(types::TunnelSessionAudit::from).collect()`
  ——映射在 types.rs `From<p2p_tunnel::TunnelAuditRecord>`，tunnel_open（§19.3-9，
  token=""）复用同一路径即可。
- `#[tauri::command]` 薄包装（tunnel.rs）未动，命令表两段路径字面保持。

## 给 TC 的移交注记（p2pctl tunnel connect）

- 直接消费 `TunnelOpener`（生产 impl = `TunnelClient` 包装，参考 GUI NodeTunnelOpener：
  TunnelClient::new(stream_factory) + TunnelTicket::new(uid, target, nonce)）
  + `LocalProxy::bind(target_port, peer_id, opener)` + `local_addr()`/`serve()`。
- 会话审计读 `ctx.audit_snapshot()`：存续期 `ended_at == 0` 哨兵 + 占位 Served
  （漂移①）；headless 面如需 "open" 语义自行按哨兵判定。
- CLI 侧注意：`TunnelAuditOutcome` 穷尽匹配面（serve.rs emit_stopped）不受影响——
  local_proxy 产出的记录不进 responder 的 TunnelAudit。

## 已知坑复认（本卡实测）

- spawn 参数在 spawn 前求值：serve() 内 `conn::serve_conn(tcp, &ctx)` future 构造
  无 await 参数，逐字平移安全。
- 反代测试并发对拷语义：4 条 proxy 用例全部 multi_thread flavor 原样保留，
  200ms 分拍响应断言（整包缓冲检测）在 crate 侧同样绿。
- `&Arc<SessionAudit>` → `&dyn PumpAudit`：泵调用点显式 `audit.as_ref()`，不赌
  deref+unsize 链式强转。
