# GUI 本地控制通道（GC1）

> 状态：已实现（feat/gui-control-channel）。CLI 侧消费在 GC2（p2pctl gui 命令域）。
> 本文件是设计契约登记；gui-contract.md 与 docs/ops/cli-guide.md 不受本单影响。

## 1. 目标与边界

用户裁定「CLI 应能截图/录屏/操作当前已打开的 GUI」。本单在 apps/gui/src-tauri 内
内聚实现一条**仅本机可达**的控制通道，随 GUI 启动开启、随 GUI 退出停止。

红线（本单已遵守）：

- 不向 generate_handler![...] 新增任何命令（CL4 的 cli-parity 守卫按该清单机械扫描，
  私加命令会导致 CLI 对等守卫假红）。控制通道是独立 HTTP 模块，与 Tauri IPC 面平行。
- 不触碰 apps/cli/**、crates/**、根 Cargo.toml、Makefile、scripts/**、docs/ops/**。

## 2. 传输与发现

- 传输：本机 HTTP/1.1 + JSON（tiny_http），仅绑 127.0.0.1，禁止对外网卡。
- 端口策略：默认 7819；被占用时 warn 日志后回退临时端口。环境变量
  P2P_CONTROL_PORT 显式指定端口时，被占用直接报错（不静默换口）。
- 端点状态文件：<GUI 数据目录>/control/endpoint.json，成功绑定后写入：
  { "http": "127.0.0.1:<port>", "pid": <pid>, "version": "<pkg version>",
    "startedAtMs": <ms>, "tokenFile": "control/token" }
  GUI 退出（RunEvent::Exit）时摘除该文件；进程崩溃残留的文件可用 pid 探活甄别。
- token：<GUI 数据目录>/control/token，首跑生成（OS 随机 32B hex），
  文件权限 600（unix，建文件即 600，存量文件权限纠偏）。

macOS 数据目录：~/Library/Application Support/com.p2p.console/control/。

## 3. 鉴权

- 所有端点要求 Authorization: Bearer <token>。
- 缺 token / 错 token 一律 401 UNAUTHORIZED（恒时比较，不区分缺失与错误）。
- token 明文仅存于本机数据目录；CLI 从 token 文件读取，不要求用户传参。

## 4. 端点

成功响应：200 {"ok": true, "data": ...}；失败：{"ok": false, "error": {"code", "message"}}。

| 端点 | 方法 | 请求 | 成功 data | 错误码 |
|---|---|---|---|---|
| /health | GET | - | {version,title,route,pid,uptimeMs,recording} | UNAUTHORIZED |
| /screenshot | POST | {"path": 绝对路径} | {path,width,height,bytes} | CAPTURE_PERMISSION_DENIED(403) / CAPTURE_UNAVAILABLE(503) / CAPTURE_FAILED(500) / SAVE_FAILED(500) / INVALID_REQUEST(400) |
| /record/start | POST | {"path": 绝对路径, "intervalMs"? 200..5000 默认 500} | {path,intervalMs} | RECORD_CONFLICT(409) / RECORD_START_FAILED(500) |
| /record/stop | POST | - | {path,frames,bytes,truncated} | RECORD_NOT_ACTIVE(409) / RECORD_EMPTY(500) |
| /navigate | POST | {"route": 路由名} | {route,path} | INVALID_ROUTE(400) / INVALID_REQUEST(400) |
| /invoke | POST | {"command","args"?} | {result} | INVOKE_FORBIDDEN(403) / INVOKE_FAILED(500) |
| /page/current | GET | requestId?（query，省略则服务端生成） | {schemaVersion,page,descriptor} | 见 §9 |
| /page/action | POST | {page,action,args?,requestId} | {requestId,result} | 见 §9 |

通用错误：UNAUTHORIZED(401) / INVALID_REQUEST(400) / NOT_FOUND(404) / METHOD_NOT_ALLOWED(405)。
请求体上限 1 MiB。

## 5. 原语语义

- **screenshot**：捕获主窗口 webview 内容（WKWebView takeSnapshot，非整屏抓取），
  PNG 原子落盘（临时文件 + rename）。落盘前校验 PNG magic 与非空，失败不产文件。
- **record**：定时采样帧（默认 500ms）编码 GIF（格式不限的最小闭环，免 ffmpeg）。
  帧最长边降采样至 640；300 帧上限（truncated=true 可观测）。落盘与校验同 screenshot。
- **navigate**：Rust 侧 eval window.location.hash（HashRouter 原生响应），
  路由名白名单校验（dashboard/peers/discovery/relay/chat/events/settings/diagnostics，
  与 menu.def.ts / App.tsx 对齐，"/" 记作 dashboard）。前端侧 lib/control-bridge.ts
  监听 hashchange 将当前路由经 control-route 事件上报，health.route 实时反映。
- **invoke**：显式白名单 = generate_handler! 的只读子集：
  node_status / metrics_get / metrics_history / config_get / profile_get。
  写操作（save/dial/reset 等）与任意命令名一律 403。白名单在
  src-tauri/src/control/invoke_allow.rs 显式枚举，不在 generate_handler 侧扩面。

## 6. 失败路径（R3 矩阵）

| 场景 | 行为 |
|---|---|
| macOS 屏幕录制权限缺失 | CGPreflightScreenCaptureAccess 预检，返回 CAPTURE_PERMISSION_DENIED + 授权路径人话提示；不截图不落盘，杜绝黑图 |
| 通道端口被占用 | 默认端口 warn 后回退临时端口；显式指定端口则报错；GUI 主功能不受影响（启动失败仅 error 日志 + stderr 告警） |
| 输出目录不可写 / 空字节 | SAVE_FAILED / RECORD_EMPTY，临时文件清理，不产出空文件 |
| 快照回调超时 | 闭包仅发起快照不等待；服务线程 5s 外层超时放弃（回调未达同径），CAPTURE_FAILED |
| token 文件损坏 | warn 后重新生成 |
| GUI 退出时录屏未停 | RunEvent::Exit 收尾停录屏；未产出文件留 error 日志 |

## 7. 测试映射（R5）

集成测试 tests/control_channel.rs（mock runtime + 合成帧源，真实 HTTP 全链路）：

- 无/错 token 拒绝 -> missing_token_rejected / wrong_token_rejected
- health 合法 JSON -> health_returns_valid_json
- screenshot 非空 PNG（临时目录）-> screenshot_writes_nonempty_png
- 失败路径不产文件 -> screenshot_rejects_bad_requests_without_file
- record start/stop -> record_start_stop_produces_gif
- navigate 路由切换 -> navigate_switches_route_and_rejects_unknown
- 未授权 invoke 拒绝 -> invoke_whitelist_forward_and_reject
- token 600 + endpoint 可发现 -> token_file_0600_and_endpoint_discoverable

说明：macOS 真实快照依赖主线程 webview，headless cargo test 无法构造，
以合成帧源覆盖截图/录屏管线的编码/校验/落盘路径；真实快照路径由 GUI 运行态验证。

## 8. GC2 衔接

p2pctl 预期流程：读 control/endpoint.json 探活（pid 校验）-> 读 control/token ->
Bearer 调用上述端点。本通道串行处理请求，CLI 侧无需并发。

## 9. 页面语义协议（GC3）

> 状态：已实现（feat/page-registry）。前端注册表先落 chat/peers/settings 三页示范（R5 裁定）；
> dashboard/diagnostics/discovery/events/relay 路由名合法但未登记，协议端结构化拒绝，待 GC3b 补全。

### 9.1 descriptor schema（schemaVersion = 1）

```json
{
  "name": "chat",
  "description": "IM 聊天页：……",
  "actions": [
    {
      "name": "sendText",
      "description": "向好友发送文本……",
      "args": [
        { "name": "peer", "type": "string", "required": true, "description": "好友 PeerId" }
      ],
      "confirm": true
    }
  ],
  "state": { "selectedPeer": "...", "friends": [ ... ] }
}
```

- arg type ∈ string | number | boolean | array | object。
- `confirm: true` 为危险动作标记（identity reset / 移除好友类）：调用方必须传
  `args.confirm === true`，否则 ACTION_CONFIRM_REQUIRED(400) 拒绝。
- actions 与页面按钮同源（store / IPC 直调），不做 DOM 模拟点击。
- `state` 可选：describe 时实时采样的页面状态快照（如 peers 表格同口径行）。

### 9.2 端点

| 端点 | 请求 | 成功 data | 错误码 |
|---|---|---|---|
| GET /page/current | requestId?（query） | {schemaVersion, page, descriptor} | PAGE_NOT_REGISTERED(404) / PAGE_TIMEOUT(500) |
| POST /page/action | {page, action, args?, requestId} | {requestId, result} | PAGE_NOT_FOUND(404，页名不在路由白名单) / PAGE_NOT_REGISTERED(404，路由合法但前端未登记) / ACTION_NOT_FOUND(404) / ACTION_CONFIRM_REQUIRED(400) / ARG_MISSING(400) / ARG_TYPE_MISMATCH(400) / ACTION_FAILED(500) / PAGE_TIMEOUT(500) / INVALID_REQUEST(400) |

- page/action 的 page/action/requestId 三个字符串字段必填；args 缺省按 {} 处理。
- GET /page/current 恒以当前路由（health.route 同源）为准，不接受指定页。

### 9.3 事件与超时语义

- server → 前端：webview eval `window.__P2P_PAGES__ && window.__P2P_PAGES__.request('<json>')`，
  载荷形如 {requestId, kind: "describe"|"execute", page, action?, args?}；
  JSON 双次序列化成 JS 字符串字面量，天然防注入。
- 前端 → server：Tauri 事件 `control-page-reply`，载荷 {requestId, ok, data? | error?}；
  server 按 requestId 关联在途请求，未注册 action / 页面未登记等拒绝由前端结构化回执。
- 超时：server 等待回执上限 5s；超时返回 PAGE_TIMEOUT(500)，报错含关联 id，不静默。
  超时后迟到的回执按无主回执丢弃并留 warn 日志。
- 测试可见性：mock runtime 下无 webview，集成测试以同事件名/同载荷形状注入回执
  （tests/control_page.rs），HTTP 链路真实。

### 9.4 测试映射（GC3）

- 前端 vitest：注册表结构校验 / 危险动作清单 / chat+peers descriptor 快照 /
  describePage 状态快照 / executePageAction 拒绝路径与真实执行器（page-registry.test.ts，
  page-bridge.test.ts 覆盖事件回执形状）。
- cargo tests/control_page.rs：describe 回包 / action 执行回包 / PAGE_TIMEOUT /
  页名+缺字段服务端拒绝 / 前端拒绝码 HTTP 映射 / 未登记页 404。
