# 任务 W-T2：tunnel 被访侧（新 crate `p2p-tunnel` 协议实现 + itest 红绿）

类型：实现（code-quality 门：每步红绿双向，injection 证明测试真能红）

## 目标
在 A 机（p2p 节点侧）落地 `/p2p-base/tunnel/1` 的被访侧：注册 handler、校验票据与目标白名单、拨本地 HTTP 连接、
双向字节哑泵、审计留痕；并把访侧客户端 `TunnelClient` 一并实现（访侧实现者 W-T3 直接消费你导出的接口）。

## 必读输入（指针）
- 【冻结契约】（见任务书末尾，逐字冻结；实现与它不一致即 bug）
- `crates/repair-bridge/src/lib.rs` —— 字节哑泵与 1 MiB 分块先例（**照它的克制程度写，不要发明 HTTP 语义**）
- `crates/p2p-protocol/src/lib.rs`（`ProtocolHandler` trait）、`crates/p2p-protocol/src/stream_factory.rs`（访侧开流）
- `crates/p2p/src/node.rs`（`handle_protocol` 注册缝）、`crates/p2p/src/lib.rs`（`NodeConfig`）
- `crates/llm-share-proxy/src/serve.rs` + `client.rs` —— 被访侧/访侧的装配与错误码体例先例
- `crates/p2p-itest/tests/llm_share_wave.rs` —— 双节点 itest 体例（真实两 Node、真实拨号）
- `apps/gui/src-tauri/src/llm_share/serve/mod.rs` —— 产品装配先例（handler 注册在 GUI 进程内的 Node 上）
- `Makefile` / `scripts/check/*.sh` —— 门禁口径（line-limit/panic-hygiene 只扫 `crates/**`）

## 交付清单
1. **新 crate `crates/p2p-tunnel`**（根 `Cargo.toml` 的 `[workspace.dependencies]` 与 members 只做追加）：
   - `wire.rs`：`PROTOCOL_ID`、`TunnelTicket`（encode/decode，字段与取值域严格照契约 §2）、`TunnelErrorCode`（六码 wire 字面量）
   - `responder.rs`：`TunnelResponder<H: HttpDialer>` 实现 `ProtocolHandler`（被访侧）
   - `client.rs`：`TunnelClient<S: StreamFactory>`（访侧 `open(peer, ticket) -> TunnelIo`）
   - `config.rs`：白名单（`127.0.0.1:<port>` 精确匹配，默认空）、按次开启开关（会话态默认关闭）、并发上限、超时参数
   - `audit.rs`：会话审计记录（字段照契约 §5）
2. **`apps/gui/src-tauri` 装配**：把 responder 注册到 GUI 进程内 Node 的 handler 表（默认关闭、需显式开启）；
   被访侧开关与白名单以 **Rust 内部 API** 提供（前端命令面由 W-T3 负责，你**不要**碰 `apps/gui/src/**` 前端与 `lib.rs` 的 Tauri 命令表之外的命令注册；如需新增命令先报 NEEDS_CONTEXT）。
3. **itest**（`crates/p2p-itest/tests/tunnel_wave.rs`，真实两 Node）：
   - 绿：`open → ticket → ack → 双向字节往返`（含 >1 MiB 的单向大流量与 ≤64 KiB 分块合并断言）
   - 绿：HTTP/1.1 形态字节流能原样过隧道（用固定字节夹具即可，**不需要真起 HTTP 服务**）
   - 红：非白名单目标 → `target_not_allowed` 错误帧（不是静默断流）
   - 红：按次开关关闭时建流 → 被拒且审计有记录
   - 红：过期 `ts` / 未开启 / 版本不符票据 → `bad_ticket`
   - 红：并发超限 → `busy`
   - 半关闭：一侧 finish 后另一方向仍能继续传，双向都 finish 后整隧道关闭
4. **注入实验**（红绿双向证据）：至少对"白名单校验"与"按次开启"两处，各做一次"故意改坏 → 对应用例必红 → 还原 → 绿"的记录。

## 验收标准（全部满足才可报 DONE）
- [ ] `cargo test -p p2p-tunnel` 与 `cargo test -p p2p-itest --test tunnel_wave` 全绿（贴命令 + 退出码 + 用例数）。
- [ ] `cargo clippy -p p2p-tunnel --all-targets -- -D warnings` exit 0；`bash scripts/check/line-limit.sh` PASS；`bash scripts/check/panic-hygiene.sh` PASS。
- [ ] 拒绝路径证据：每条红用例贴出**实际收到的错误帧/错误码**（禁止只贴"测试通过"）。
- [ ] 审计证据：跑一次成功会话 + 一次被拒会话，贴两条审计记录的字段内容。
- [ ] 注入实验记录：两处"改坏→红→还原→绿"的命令与输出。
- [ ] 重活门禁（`make check`）**不要在 worktree 里跑**（W1 正在改 `scripts/check/**`，跑了会产生假红混淆）；只跑你域内的轻量自验，全量门禁由协调者在合并后执行。
- [ ] worktree 内 `git rebase main` 后 push `origin feat/wt2-tunnel-responder`；**不要自行合并 main**。

## 边界（明确不做）
- **不做访侧本地 HTTP 反代、不碰 `apps/gui/src/**` 前端、不做 Tauri 命令与事件**（W-T3 的域）。
- 不改 `scripts/check/**`、`Makefile`、`.github/**`（W1 域）；不改 `docs/protocol/**`（W2 与 W-T1 域）——
  契约已在任务书里，你不需要改文档；如需登记行由 W-T1 负责。
- 不改 `crates/p2p-protocol` 的既有 trait 与帧封装语义（只消费）；不改 `apps/gui/src-tauri/src/lib.rs` 的既有命令表结构（只追加注册行）。
- 不解析 HTTP 语义（不处理 hop-by-hop、不改请求行/头；Host 重写是访侧的事）。
- 不引入新依赖，除非确有必要且报明理由（优先复用 workspace 现有 crate）。
- 不改 DSH 仓库（`/Users/imeepos/ext512/ymm-001/deepseek-harness`）任何文件。

## 接口契约
- Consumes：main @ `4df3eb5`；【冻结契约】§1-§5、§7；`p2p-protocol` 的 `ProtocolHandler` / `StreamFactory` / `read_frame|write_frame|read_chunked|write_chunked`。
- Produces（W-T3 直接消费，名称与语义不得偏离契约 §7）：
  `p2p_tunnel::{PROTOCOL_ID, TunnelTicket, TunnelErrorCode, TunnelResponder, HttpDialer, TunnelClient, TunnelIo, LocalProxy 接口}`；
  被访侧在 GUI 内的开关/白名单 Rust 内部 API。

## 预算与停止条件
- 预算：约 4 轮修复 / ≤60 次工具调用（先写红用例，再实现）。
- 报 **BLOCKED**：契约在实现中不可行（例如半关闭语义与底座帧封装冲突）；或必须改 `p2p-protocol` 既有语义。
- 报 **NEEDS_CONTEXT**：契约 §7 导出的接口签名不足以让 W-T3 并行工作（缺类型/缺错误映射）。

## 工作区与汇报
- `git worktree add .worktrees/wt2-responder -b feat/wt2-tunnel-responder 4df3eb5`；先落 `PROGRESS.md`（勿提交）。
- 只 add 自己范围文件，禁 `git add -A`；message `type(scope): subject` + 写机理；feat 带测试、fix 带回归。
- 汇报用 `session_link_send_parent`；异常时 `session_link_send` 到 `session-3373f897-f9c0-4a32-9af2-b9562eecc311`。
- 状态只用四枚举；DONE 附：文件清单（逐文件 `wc -l`）、每条验收的命令+退出码+关键输出、注入实验记录、
  Export 面清单（W-T3 消费所需的类型与函数签名逐条列出）、顾虑、结构化复盘（最耗时 / 任务书是否提前警告 / 重来怎么做）。

---

# 【冻结契约】`/p2p-base/tunnel/1`（协调者冻结，逐字生效）

## 1. 协议 ID 与帧序
- 协议 ID：`/p2p-base/tunnel/1`（常量唯一定义点放 `p2p-tunnel`）。
- 帧序：**首帧 = 票据帧（JSON UTF-8，≤4 KiB）→ 一帧应答（`ack` 或 `error`）→ 双向字节流分块**；首帧之后全部走底座帧封装（单帧上限 1 MiB）。
- 首帧超时：建流后 **5s** 内未收到合法票据即拒（`bad_ticket`）并关流。
- **分块合并**：出站分块每次载荷 ≤ **64 KiB**；读侧必须容忍任意 ≤1 MiB 边界。

## 2. 票据帧 `TunnelTicket`
| 字段 | 类型 | 语义 |
|---|---|---|
| `v` | u8 | 固定 `1`；未知版本即 `bad_ticket` |
| `uid` | string | 会话 id（16 hex，访问侧生成，两侧日志同源） |
| `target` | string | `127.0.0.1:<port>`；**只允许回环字面量 `127.0.0.1`**，其他 host 一律 `target_not_allowed` |
| `nonce` | string | 32 hex 随机；仅作会话关联与重放检测，**不是鉴权凭据** |
| `ts` | u64 | 访问侧 unix 秒；窗口 ±300s，越界即 `bad_ticket` |

身份来自底座握手 PeerId；**票据内不得自报身份，也不得采信自报身份**。

## 3. 应答帧
- `{"k":"ack","uid":"<同请求>"}`
- `{"k":"error","code":"<code>","msg":"<可选>"}`
- 错误码闭集：`bad_ticket` / `target_not_allowed` / `busy` / `dial_failed` / `io` / `shutdown`
- 拒绝路径必须显式回 `error` 帧，禁止静默断流。

## 4. 数据面语义
- `ack` 后进入字节流阶段，两方向独立流动（请求体与响应体互不阻塞）。
- `finish()` = 本方向半关闭；两侧均 finish 后整条隧道关闭。
- 任一侧 IO 错误/超时 → 整隧道关闭；关闭原因落审计（含 code）。
- WebSocket 升级：访侧完成 HTTP 101 后，升级 socket 与该流做裸字节双向泵。

## 5. 准入与审计
- 被访侧：目标白名单**显式配置**（`127.0.0.1:<port>` 精确匹配），默认空 = 全拒；**按次开启**（会话态，默认关闭）。
- 访侧本地监听**只绑 `127.0.0.1` 字面量**。
- 审计字段：`session_id / peer_id / target / started_at / ended_at / bytes_in / bytes_out / outcome`。

## 6. 访侧本地反代行为（W-T3 用）
- **重写 `Host` 头为目标 `127.0.0.1:<port>`**，其余 method/path/headers/body 原样过隧道。
- 响应与 body **流式转发**，禁止整包缓冲。
- DSH 启动 URL 解析：`http://127.0.0.1:<port>/?token=<tok>` → `local_addr` + `open_url = http://127.0.0.1:<local_port>/?token=<同 token>`。

## 7. 冻结接口
- `p2p-tunnel`：`PROTOCOL_ID`、`TunnelTicket`、`TunnelErrorCode`、`TunnelResponder<H: HttpDialer>`（impl `ProtocolHandler`）、
  `trait HttpDialer { async fn dial(&self, target:&str) -> Result<TunnelIo, TunnelError>; }`、
  `TunnelClient<S: StreamFactory>`（`open(peer, ticket) -> TunnelIo`）、`LocalProxy<C>`（`bind(127.0.0.1:0)` + 回传 `local_addr`）。
- `apps/gui/src-tauri`：命令 `tunnel_open_dsh(url) -> { local_addr, open_url, token }`、命令 `tunnel_status()`、事件 `tunnel_status`（W-T3 实现）。
