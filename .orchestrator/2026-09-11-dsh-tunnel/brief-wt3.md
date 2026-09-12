# 任务 W-T3：tunnel 访侧（本地反代 + GUI「远程访问」入口 + 本机端到端实证）

类型：frontend + 实现（GUI 面必须出截图证据链与三态齐全；Rust 面按实现类验收）

## 目标
在 B 机（本机 GUI）落地隧道访侧：本地回环反代（Host 重写 + 流式转发 + WebSocket 升级裸泵）+ GUI「远程访问」入口
（粘贴 A 机 `dsh web` 启动 URL → 解析 port/token → 生成入口链接 → 系统浏览器打开），并证明"本机 DSH 经隧道能被完整打开"。

## 必读输入（指针）
- 【冻结契约】（任务书末尾，逐字冻结；§6 与 §7 是你这一侧的全部行为定义）
- `crates/acp-pump/src/ws.rs` + `pump.rs` + `dial.rs` —— **本仓最贴近的骨架**：本地回环服务 + token + 一连接一泵
- `apps/gui/src-tauri/src/control/server.rs` + `paths.rs` —— tiny_http 回环服务的既有范式（端口策略、Bearer、endpoint 落盘）
- `apps/gui/src-tauri/src/console/mod.rs` + `types.rs` —— 进程内装配后把 addr/token 句柄回传前端的先例
- `apps/gui/src-tauri/src/lib.rs`（Tauri 命令表）、`apps/gui/src-tauri/Cargo.toml`（`tiny_http` 已依赖）
- `apps/gui/src/views/` 任一既有页 + `apps/gui/src/i18n/locales/{zh-CN,en-US}.ts` + `apps/gui/src/lib/menu.def.ts` —— 新页面/菜单项登记体例（**登记类改动压独立小提交**）
- `apps/gui/src/lib/open-conversation-window.ts` —— WebviewWindow 开窗先例（若做内嵌视图）
- `docs/design/gui-contract.md` —— 你实现的两个命令的字段以 W-T1 登记的 §19 为准（W-T1 在途；若未落地，以契约 §7 形状为准并在汇报注明）

## 交付清单
1. **Rust 访侧反代**：在 `apps/gui/src-tauri` 内（或消费 `p2p-tunnel` 的 `LocalProxy`）实现：
   - 本地 HTTP 服务只绑 **`127.0.0.1` 字面量**（端口 0 由 OS 分配，句柄回传）；
   - 每请求：**重写 `Host` 为目标 `127.0.0.1:<port>`** → 走 `TunnelClient` → **流式回写响应**（禁整包缓冲）；
   - WebSocket/`Upgrade: websocket`：完成 101 后把升级 socket 与隧道流做裸字节双向泵。
2. **两条 Tauri 命令 + 事件**（照契约 §7，并遵守下方边界里的**精确插入位置**）：
   `tunnel_open_dsh(url) -> { local_addr, open_url, token }`、`tunnel_status()`、事件 `tunnel_status`。
3. **GUI 前端**：新「远程访问」视图（粘贴 URL 表单 + 开启/关闭 + 本地地址展示 + 三态：未开启/已开启/错误）
   + 菜单/路由登记（`menu.def.ts` 与路由表）+ i18n 中英词条；i18n 键先独立小提交。
4. **本机端到端实证（必交）**：本机起 `dsh web --no-open` → 把打印的 URL 粘进 GUI → 系统浏览器打开 `open_url` →
   证明：首屏加载、`/api/remote.mux` 握手成功、一条消息流式返回；负面：用过期 token 走一次，验证是明确 401 提示而非白屏。

## 验收标准（全部满足才可报 DONE）
- [ ] `cd apps/gui && pnpm lint && pnpm build && pnpm test --run && pnpm check:i18n` 全绿（贴四条命令与退出码）。
- [ ] `cd apps/gui/src-tauri && cargo test && cargo clippy --all-targets -- -D warnings` exit 0。
- [ ] **本机端到端**：贴出证据链——`dsh web` 启动行的实际输出、GUI 视图截图（含三态各一张）、
      浏览器端首屏截图、浏览器 console 无 error 的截图或输出、`/api/remote.mux` 握手成功的可观测信号、
      一条消息流式增量出现的截图（证明未被整包缓冲）。
- [ ] **负面**：过期/错误 token 的一次实测（截 401 明确提示的画面），并说明提示文案来自哪里。
- [ ] 本地监听校验：`lsof -nP -iTCP -sTCP:LISTEN | grep <local_port>` 只出现在 127.0.0.1（贴输出）。
- [ ] Host 重写校验：贴出可复现的观测（例如临时开 debug 日志或用一个本地 echo 服务捕获收到的 `Host` 头）。
- [ ] 行数红线：新增 Rust 文件 ≤300 行、函数 ≤60 行（贴 `wc -l`）；`bash scripts/check/panic-hygiene.sh` PASS（非测试路径无 unwrap/expect/panic）。
- [ ] 轻活自验即可；**不要在 worktree 跑 `make check`**（W1 在途改 `scripts/check/**`，全量门禁由协调者合并后执行）。
- [ ] worktree 内 `git rebase main` 后 push `origin feat/wt3-tunnel-client`；**不要自行合并 main**。

## 边界（明确不做）
- **不做被访侧**（handler/白名单/审计是 W-T2 域）；不新建 `crates/p2p-tunnel` 内的文件（只消费其导出）。
- 不改 `scripts/check/**`、`Makefile`、`.github/**`（W1 域）；不改 `docs/protocol/**`（W2/W-T1 域）；
  不改 `docs/design/gui-contract.md`（W-T1 登记）。
- `apps/gui/src-tauri/src/lib.rs` 的命令表**只允许追加两行**（`tunnel_open_dsh`、`tunnel_status`），
  **插入位置精确为：`authz::authz_default_role_save,` 一行之后、`])` 之前的最后两行**（W-T2 也在同一文件追加它的两行，位置错开即 ff 冲突）。
  该文件的 `invoke_handler` 结构、既有命令顺序**不得改动**。
- 不做内嵌 Webview/iframe 视图（第二里程碑）；本卡只做"系统浏览器打开"路径。
- 不改 DSH 仓库（`/Users/imeepos/ext512/ymm-001/deepseek-harness`）任何文件；DSH 侧零改动是本轮的硬约束。
- 不以 mock 代替真实链路证据：本机端到端必须真起 `dsh web` 并真连一次（mock 只用于单测）。

## 接口契约
- Consumes：main @ `4df3eb5`；【冻结契约】§1、§3、§4、§6、§7；`p2p-tunnel` 的 `TunnelClient`/`TunnelTicket`/`TunnelErrorCode`/
  `LocalProxy`（W-T2 提供，若尚未合并，先在你自己分支按契约 §7 的签名写消费侧代码，并在汇报里标记该依赖为"按契约并行"）。
- Produces：GUI「远程访问」能力与两条命令 + 事件（W-T1 的 gui-contract §19 与你的实现必须字面一致）。

## 预算与停止条件
- 预算：约 4 轮修复 / ≤70 次工具调用。
- 报 **BLOCKED**：本机 `dsh web` 无法启动（如 node 版本/dsh 不可用）；或契约 §7 接口缺失导致访侧无法实现（先要 W-T2 的导出面）。
- 报 **NEEDS_CONTEXT**：Host 重写后仍被 DSH 403/401 拒绝，且你没有可复现证据判断是重写位置错还是契约缺陷（附请求头与响应头原文）。

## 工作区与汇报
- `git worktree add .worktrees/wt3-client -b feat/wt3-tunnel-client 4df3eb5`；先落 `PROGRESS.md`（勿提交）。
- 只 add 自己范围文件，禁 `git add -A`；i18n/菜单登记类改动独立小提交；message `type(scope): subject` + 写机理。
- 汇报用 `session_link_send_parent`；异常时 `session_link_send` 到 `session-3373f897-f9c0-4a32-9af2-b9562eecc311`。
- 状态只用四枚举；DONE 附：文件清单（逐文件 `wc -l`）、每条验收的命令+退出码+关键输出、截图清单与路径、
  Host 重写与流式转发的观测证据、顾虑、结构化复盘（最耗时 / 任务书是否提前警告 / 重来怎么做）。

---

# 【冻结契约】`/p2p-base/tunnel/1`（协调者冻结，逐字生效）

## 1. 协议 ID 与帧序
- 协议 ID：`/p2p-base/tunnel/1`；帧序：**首帧票据（JSON ≤4 KiB）→ 一帧应答（ack/error）→ 双向字节分块**（底座帧封装，单帧 ≤1 MiB）。
- 首帧 5s 超时即 `bad_ticket`；出站分块 ≤64 KiB，读侧容忍 ≤1 MiB 边界。

## 2. 票据 `TunnelTicket`
`v`(u8,=1) / `uid`(16 hex) / `target`(`127.0.0.1:<port>`，只允许回环字面量) / `nonce`(32 hex，非凭据) / `ts`(unix 秒，窗口 ±300s)。
身份取底座握手 PeerId，票据不得自报身份。

## 3. 应答
`{"k":"ack","uid":"…"}` 或 `{"k":"error","code":"…","msg":"…"}`；
错误码闭集 `bad_ticket` / `target_not_allowed` / `busy` / `dial_failed` / `io` / `shutdown`；拒绝必须显式回 error 帧。

## 4. 数据面
`ack` 后两方向独立流动；`finish()` 为半关闭，双向 finish 后整隧道关闭；IO 错误即整隧道关闭并落审计。
WebSocket：101 后升级 socket 与流做裸字节双向泵。

## 5. 准入与审计（被访侧）
目标白名单显式配置（默认空=全拒）+ 按次开启（默认关闭）；访侧监听只绑 `127.0.0.1` 字面量；
审计字段 `session_id / peer_id / target / started_at / ended_at / bytes_in / bytes_out / outcome`。

## 6. 访侧行为（本卡）
**重写 `Host` 头为目标 `127.0.0.1:<port>`**，其余原样过隧道；响应与 body 流式转发禁整包缓冲；
DSH URL `http://127.0.0.1:<port>/?token=<tok>` → `local_addr` + `open_url = http://127.0.0.1:<local_port>/?token=<同 token>`。

## 7. 冻结接口
- `p2p-tunnel`：`PROTOCOL_ID` / `TunnelTicket` / `TunnelErrorCode` / `TunnelResponder<H: HttpDialer>`（impl `ProtocolHandler`）/
  `trait HttpDialer` / `TunnelClient<S: StreamFactory>`（`open(peer, ticket)`）/ `LocalProxy<C>`（`bind(127.0.0.1:0)`）。
- `apps/gui/src-tauri`：命令 `tunnel_open_dsh(url) -> { local_addr, open_url, token }`、`tunnel_status()`、事件 `tunnel_status`。
