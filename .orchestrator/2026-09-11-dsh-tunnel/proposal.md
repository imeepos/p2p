# 可行性评估方案：p2p 隧道访问 DSH Web（2026-09-11）

状态：**待用户商定**（用户明确指示：未确认前不派发任何任务）
事实基线：main 4df3eb5；DSH checkout `/Users/imeepos/ext512/ymm-001/deepseek-harness` 0.1.5-rc.2
支撑调研：DSH 侧只读调研（路由/鉴权/启动载荷）+ p2p 侧只读调研（可复用接缝/缺口）

## 1. 结论

**可实现，且不需要改 DSH 源码。** 但只有一种自然形态：访问侧起本地回环入口，把 `Host` 重写成远端 DSH 的 authority
（`127.0.0.1:<远端端口>`）后经 p2p 字节隧道直达远端 DSH。除此以外的任何"改域名/加 trusted-host/挂子路径"路线
都会被 DSH 的鉴权设计打回（见 §2 第 3 组事实）。

## 2. 决定性事实（证据见两份调研报告）

A 组 · DSH Web 是什么：
- 原生 `node:http`，默认 `127.0.0.1:3080`；`--host 0.0.0.0` 被 CLI **显式拒绝**（"would expose RCE to the network"）。
- 载体 = 同源 HTTP + 一条 WebSocket（`/api/remote.mux`）+ 一条 SSE（`/plugins/events`）+ chunked 流式 RPC。
- 前端后端地址**全靠同源推导**（`location.origin`），`__DSH_BOOT__` 不含任何地址/端口。

B 组 · 鉴权：
- 进程内一次性随机 token（重启失效）→ `GET /?token=` 换 **HMAC-SHA256 cookie**；cookie 名与 payload.authority
  **都绑定请求 `Host`**；`HttpOnly; SameSite=Strict; Path=/`。
- `/api` 另有 Host/Origin 围栏：Host 必须是回环或 `trustedHosts`；`sec-fetch-site: cross-site` 一律拒。

C 组 · 为什么必须 Host 重写 + 同源回环：
- 换任何入口 authority 就换 cookie 命名空间 → 必须重新走一次该 authority 的 `?token=`；token 又只有启动行里有。
- `<base href="/">` 强制站点根，子路径部署全线打错前缀。
- 因此：**让浏览器始终认为自己在访问 `http://127.0.0.1:<本地端口>`，隧道内侧把 Host 改写成远端 authority**，是唯一
  同时满足 cookie 绑定、Host 围栏、同源推导、根路径四条约束的姿态。

D 组 · p2p 侧现状：
- **无任何通用 HTTP 代理/隧道/端口转发**；协议注册表 16 条全 implemented，无隧道类 ID 占位。
- 可复用接缝 5 条：协议 handler 注册（不改内核）、WS ⇄ P2P 流哑泵（acp-pump::pump）、回环 HTTP+token+endpoint.json
  范式（control/）、进程内装配句柄回传前端、帧封装/chunked 1MiB 设施。
- 缺口 5 条：通用 HTTP 语义层、本地监听→拨号转发组件、hop-by-hop 头与背压约定、被共享服务的准入模型、
  GUI 内嵌浏览/外链消费路径（`csp=null` 但 assetProtocol scope 仅 chat/media，opener 仅 github 白名单）。

E 组 · 环境既有资产：`~/.dsh/dshplug/` 有已构建未装的 `@waixiaowai/dsh-host-p2p-bridge@0.1.0-rc.32` 与
`@waixiaowai/dsh-client-ui-p2p@0.1.0-rc.1`（来电式人工授权 + Remote 操作 + Web 界面插件），与
`P2P_PAIRING_FOLLOWUP_PROMPT.md` 描述同族——需用户裁定是否纳入本需求。

## 3. 三个备选形态

### 形态 A（推荐）：p2p 通用字节隧道 + GUI 远程访问入口
- 被访问侧（A 机）：p2p 节点注册新协议 handler `/p2p-base/tunnel/1`，入站流带首帧票据（目标 `127.0.0.1:<port>` + 一次性 token）；
  校验通过后拨本地 HTTP 连接并做双向哑泵（复用 repair-bridge 范式的字节层，但**方向相反**：p2p 流是发起端）。
- 访问侧（B 机）：GUI Rust 侧起 `127.0.0.1:<本地随机端口>` 的 tiny_http 反代，把请求**改写 Host** 后经隧道转发，
  WS 升级按裸 socket 双向泵。
- 消费路径：先做"复制链接 + 系统浏览器打开"（零前端改动，第一个可验收里程碑）；第二步做 GUI 内嵌视图。
- 优点：DSH 零改动；通用能力（任何本地服务都能共享）；与现有协议/接缝同构。缺点：新协议 ID + 新 crate + GUI 面。

### 形态 B：复用 DSH 插件体系（`dsh-host-p2p-bridge` 路线）
- 在 DSH 侧装/推进 p2p 插件，由插件在 DSH 进程内暴露隧道；GUI 只做消费。
- 优点：与 DSH 的插件生命周期、授权 UI（来电式人工授权）天然结合。缺点：跨仓耦合；需先摸清 rc.32 的实际能力
  （是否已含 HTTP 隧道，还是只有 Remote RPC 操作面）；p2p crate 与 DSH plugin 的协议栈如何对齐未定。

### 形态 C：最小验证（不做产品化）
- 用 `ssh -L`/cloudflared 之类现成隧道 + 手工 Host 改写，或用 `dsh web --trusted-host` 配合已有云隧道，
  只验证"远端 GUI 能打开 A 机 DSH"，作为后续方案的可行性证据。
- 优点：一天内可出结论。缺点：不落产品能力、跨互联网需第三方服务、安全边界临时。

## 4. 推荐路线（形态 A，分三波）

- **W-T1 规格与契约（architecture）**：定 `/p2p-base/tunnel/1` 线格式（首帧票据字段、目标白名单、字节泵语义、EOF/半关闭、
  错误码）、准入模型（引用既有好友/角色 authz 与一次性分享票据）、本地监听器契约（仅回环、端口策略、endpoint 发现）、
  registry/spec/wire-protocol 三处登记。产出：spec 页 + ADR。
- **W-T2 被访问侧（A 机）**：handler + 目标白名单 + 审计日志 + 一次性票据；itest 两会话字节往返红绿。
- **W-T3 访问侧（B 机）**：本地反代（Host 重写 + WS 升级）+ GUI 入口（复制链接/系统浏览器）+ 后续内嵌视图；
  行为验收 = 远端 DSH 首屏加载 + `/api/remote.mux` 握手成功 + 一条消息流式返回。

依赖：W-T2/W-T3 依赖 W-T1 的 wire 契约（契约先行，可并行开工）。

## 5. 安全边界（必须先裁决，属"风险分级人审"）

- 隧道 = 把被共享服务的安全边界整体交给对端；DSH Web 本身无认证（只有启动 token + cookie）。
- 硬约束建议：被访问侧**显式按次开启**共享（默认关闭）+ 目标白名单（只允许 `127.0.0.1:<dsh端口>`）；准入走既有
  好友/角色或一次性分享票据（可复用 acp-share 的 token 只存哈希先例）；访问日志留痕（谁、何时、目标、字节量）。
- 访问侧本地监听器**只绑 127.0.0.1**，绝不对外网开放（否则等于无鉴权转发他人服务）。
- 令牌获取是唯一无法自动化的环节：`dsh web` 启动行的 `?token=` 只在 A 机屏上（进程内一次性）。
  需裁决：人工粘贴到 GUI，还是 GUI 与本地 dsh 进程做挂钩读取（后者更顺滑但要碰 DSH 侧）。

## 5b. 用户已裁决（2026-09-11）

- **形态 = A**（p2p 新通用隧道；被访侧按次开启 + 目标白名单；访侧本地回环反代 + Host 重写）。
- **令牌 = 人工粘贴**：A 机 `dsh web --no-open` 打印的 `http://127.0.0.1:<port>/?token=<tok>` 粘贴进 GUI；
  GUI 解析出 port 与 token，生成"本地代理地址 + 带 token 的一次性入口 URL"，浏览器打开入口 URL 完成 303 换 cookie（默认 30 天）。
- **验收环境 = 本机 + 102（imeepos@192.168.0.102）**。

### 由裁决推出的实现细节（写进任务书）

- 访侧本地监听必须绑 `127.0.0.1`，**且用 `127.0.0.1` 字面量而不用 `localhost`**：cookie 只看 host 不看 port，
  用 `127.0.0.1` 才能与重写后的 Host authority 精确同源；DSH 侧 `loopback-hostname.ts` 也把 `127/8` 当回环免检。
- 已核实 DSH **不设置 `X-Frame-Options` / `frame-ancestors`**（grep 无命中）→ 内嵌 Webview/iframe 可行（第二里程碑）。
- 帧层：底座 1 MiB 帧上限对 HTTP 响应与 WS 流足够，但需约定出站分块合并（建议 ≤64 KiB）避免高频小写入放大成帧风暴。

## 6. 待用户决策项（1-3 已裁决，保留其余）

1. 形态 A / B / C 选哪条（或 A+B 混合：p2p 侧做隧道、DSH 侧只出授权 UI）。
2. `~/.dsh/dshplug` 里那两个已构建插件是否纳入本需求（先跑一次能力核对再定）。
3. 验收环境：哪两台机器（本机 + 102？还是 15/99 的 trustedHosts 那两台）。
4. 令牌获取方式：人工粘贴 vs GUI 挂钩本地 dsh 进程。
5. GUI 呈现：先做"系统浏览器打开"还是一步到位内嵌 Webview。
6. 协议 ID 与流水号段预分配：`/p2p-base/tunnel/1` 是否采用（需登记 registry/spec/wire-protocol 三处）。
7. 是否允许在 DSH 仓库做改动（当前方案假设零改动；若允许，令牌获取与生命周期管理会简单很多）。

## 7. 可判定验收标准（草案）

- 两机真实链路：B 机 GUI 打开隧道入口 → A 机 DSH 首屏（含插件 bundle）渲染完整，浏览器 console 无错误。
- `/api/remote.mux` WebSocket 握手成功（`ready` 帧到达，generation 发布）；SSE `/plugins/events` 不断流。
- 会话提交一条消息，流式增量在 GUI 内逐块出现（证明 chunked 转发未被缓冲）。
- 负路径：非授权 peer 建隧道被拒（显式错误码 + 审计行）；DSH 未开启共享时隧道拒绝；
  A 机 DSH 重启后旧 token 失效表现为明确 401 提示，而非白屏。
- 门禁：新 crate `cargo test` + `clippy -D warnings` + 两会话 itest 红绿 + `make check` 全绿 + registry 门禁 PASS。
