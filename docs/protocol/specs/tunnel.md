# /p2p-base/tunnel/1 规范

状态：draft；自 2026-09-11；归属 crates/p2p-tunnel（planned，实现未合并）；符合性：Core = 票据帧
接入、应答帧与错误码闭集、双向字节分块、半关闭、准入门禁与审计、访侧 Host 重写与流式转发；
Extended = WebSocket 升级裸字节泵。

## 1. 概览

把被访节点本机 `127.0.0.1:<port>` 的 HTTP 服务经一条 p2p-base 逻辑流受限地暴露给访侧：
访侧（client）开流送 JSON 票据，被访侧（responder）校验准入后回 `ack`，此后该流退化为
双向字节隧道；访侧在本机回环起反代监听，把本地 HTTP 客户端的请求经隧道打到被访侧目标，
响应流式原路返回。典型场景：DSH 启动 URL（`http://127.0.0.1:<port>/?token=<tok>`）驱动的
远程 Web 控制台访问（docs/design/gui-contract.md §19）。

适用边界：

- 本协议只承载字节与准入应答，不解析 HTTP 语义；唯一例外是访侧反代重写 `Host` 头（§3.3）。
- 底座中继（/p2p-base/relay/1、/p2p-base/circuit/1）解决"怎么连上"；本协议解决"连上后
  如何受限访问一个回环端口"。
- 与 /repair/mcp/1 同为字节隧道，差异在票据形态与拒绝语义：本协议为结构化 JSON 票据 +
  显式 error 应答帧；repair/mcp 为内容零解释的签名凭证文本且无协议级错误帧。
- 帧封装（varint 长度前缀、单帧上限、超限断流）复用底座，见 docs/protocol/wire-format.md；
  本文只写本协议的帧 payload 语义。

## 2. 线格式

### 2.1 帧序列与超时

| 序 | 载荷 | 方向 | 语义 |
|---|---|---|---|
| 1 | 协议 ID `/p2p-base/tunnel/1` 的 UTF-8 字节（底座标准开手） | 访→被访 | 路由到本协议 handler |
| 2 | 票据帧：JSON UTF-8，恰一帧，≤ 4096 字节 | 访→被访 | TunnelTicket，见 §2.2 |
| 3 | 应答帧：JSON UTF-8，恰一帧 | 被访→访 | `ack` 或 `error`，见 §2.3 |
| 4 起 | 双向字节流分块 | 双向 | 透传字节：无类型头、无序号、无确认、无消息语义 |

- 帧序即上表：实现 MUST 以票据帧为首业务帧，被访侧 MUST 以恰一帧应答为首个反向载荷；
  顺序不得交换或省略。
- 首帧超时：建流后 **5 秒**内未收到合法票据帧，被访侧 MUST 以 `bad_ticket` 拒绝并关流。
- 访侧 MUST 先收到 `ack` 才得发送任何业务字节；收到 `error` 后 MUST NOT 再发任何字节。
- 访侧等待应答帧必须有界（超时值属实现策略，不冻结）；超时视同本次开流失败并关流。

### 2.2 票据帧 TunnelTicket

JSON 对象，UTF-8 编码，字段名逐字如下；整帧字节数 MUST ≤ 4096，超限按 `bad_ticket` 拒：

| 字段 | 类型 | 语义 |
|---|---|---|
| `v` | u8 | 固定 `1`；未知版本即 `bad_ticket` |
| `uid` | string | 会话 id（16 hex，访问侧生成，两侧日志同源） |
| `target` | string | `127.0.0.1:<port>`；只允许回环字面量 `127.0.0.1`，其他 host 一律 `target_not_allowed` |
| `nonce` | string | 32 hex 随机；仅作会话关联与重放检测，不是鉴权凭据 |
| `ts` | u64 | 访问侧 unix 秒；窗口 ±300 秒，越界即 `bad_ticket` |

- 身份来自底座握手 PeerId：票据内 MUST NOT 自报身份，被访侧 MUST NOT 采信票据内任何
  身份字段。
- `target` 为字面量精确匹配：host 段必须恰为 `127.0.0.1`（不接受 `localhost`、`::1`、
  `127.0.0.2` 等任何等价写法），端口段为 1-65535 十进制。
- JSON 未知字段 MUST 忽略（加法策略，§6）。

### 2.3 应答帧

- `{"k":"ack","uid":"<同请求>"}`：受理成功，进入数据面（§3.2）；`uid` MUST 与票据逐字一致。
- `{"k":"error","code":"<code>","msg":"<可选人读说明>"}`：拒绝；`code` MUST 取自 §4 闭集；
  `msg` 可选 UTF-8 人读说明，MUST NOT 作为机器判定依据（判定只看 `code`）。
- 拒绝路径 MUST 显式回 `error` 帧，MUST NOT 静默断流。

### 2.4 分块规则

- 出站分块每次载荷 MUST ≤ 65 536 字节（64 KiB）。
- 读侧 MUST 容忍任意 ≤ 1 048 576 字节（底座单帧上限）边界的合法帧，不得假设发送方分块大小。
- 两方向独立流动、并发对拷；背压由逐帧写出与底座流背压承担，任一方向 MUST NOT 缓冲无界增长。

## 3. 时序与状态机

### 3.1 开流时序

```
访侧（client）                              被访侧（responder）
1. 连接对端，开流写协议 ID
2. 写票据帧（§2.2，≤4096 字节）并 flush
3. 等待应答帧（有界）        →   4. 5 秒内读票据帧并校验（§2.2/§4）
                                 5. 准入门禁（§5.2：显式白名单 + 按次开启）
                                 6. dial 目标（HttpDialer）
7a. 收 ack → 进入数据面      ←      回 {"k":"ack","uid":...}
7b. 收 error → 关流并上抛    ←      回 {"k":"error",...} 后关流
```

- 被访侧处理顺序 MUST 为：读票据 → 形状/版本/时间窗校验 → 目标形状校验 → 准入门禁 →
  dial；任一步失败即按 §4 回 error 并关流，MUST NOT 跳步（如未过门禁先 dial）。
- `dial_failed` 仅表示 dial 本身失败；门禁失败 MUST NOT 伪装成 `dial_failed`。

### 3.2 数据面与半关闭（核心）

- `ack` 后进入字节流阶段，两方向独立流动（HTTP 请求体与响应体互不阻塞）。
- `finish()` = 本方向写半关；两侧均 finish 后整条隧道关闭。
- 任一侧 IO 错误/超时 → 整隧道关闭；关闭原因 MUST 落审计（§5.2，含 code）。
- WebSocket 升级：访侧完成与本地客户端的 HTTP 101 升级后，把升级 socket 与该流做裸字节
  双向泵（升级请求的反代处理同 §3.3，含 Host 重写）。

### 3.3 访侧本地反代行为（LocalProxy）

- 反代监听 MUST 只绑 `127.0.0.1` 字面量（`bind(127.0.0.1:0)` 随机端口并回传 `local_addr`）；
  MUST NOT 绑 `0.0.0.0`/`::1`/`localhost`（cookie 只看 host 不看 port，`localhost` 会与
  本机其他服务共享 cookie 域）。
- 每条本地连接 MUST 重写 `Host` 头为目标 `127.0.0.1:<port>`；其余 method/path/headers/body
  MUST 原样过隧道。
- 响应与 body MUST 流式转发，MUST NOT 整包缓冲。

### 3.4 非法情形

- 票据帧非 UTF-8 / 非 JSON 对象 / 字段缺失或类型不符 / `v` ≠ 1 / `ts` 越 ±300 秒窗 /
  超 4096 字节 / 5 秒内未到：一律 `bad_ticket` 拒绝并关流。
- `target` host 非回环字面量或形状非法：`target_not_allowed`。
- `target` 形状合法但不在显式白名单、或准入未按次开启：`target_not_allowed`。
- `ack` 前出现任何业务字节：被访侧 MUST 回 `bad_ticket` error 帧后关流。
- 底座帧层违例（超 1 048 576 字节 / varint 非法）：底座断流语义
  （docs/protocol/builtin-and-versioning.md §3.1），本协议不另定义。

## 4. 错误语义

错误码闭集（六值，MUST NOT 使用闭集外的 code；收到未知 code 的实现 MUST 关流并按 `io`
落审计）：

| code | 触发 | 检出方 | 确切行为 |
|---|---|---|---|
| `bad_ticket` | 票据缺失/非 UTF-8/非 JSON/字段不符/`v`≠1/`ts` 越窗（±300s）/超 4096 字节/5 秒超时/ack 前业务字节 | 被访侧 | 回 error 帧后关流 |
| `target_not_allowed` | host 非回环字面量/形状非法/不在显式白名单/准入未按次开启 | 被访侧 | 回 error 帧后关流 |
| `busy` | 目标合法但资源护栏拒绝（并发隧道上限等，实现配置） | 被访侧 | 回 error 帧后关流 |
| `dial_failed` | dial 目标失败（连接拒绝/超时） | 被访侧 | 回 error 帧后关流 |
| `io` | 数据面读写错误/超时 | 双侧 | 立即关整隧道，原因含 code 落审计 |
| `shutdown` | 被访侧停机中，不再受理新隧道 | 被访侧 | 回 error 帧后关流 |

- 数据面阶段没有协议级带外信令：除整隧道关闭外不产生任何控制帧；关闭原因只落审计，
  不上线。

## 5. 安全考量

### 5.1 认证与信任边界

- 机密性与对端认证完全由底座承担（QUIC TLS1.3 / Noise XX，见 docs/design/wire-protocol.md
  §5）；身份唯一来源是握手 PeerId，MUST NOT 采信票据或载荷内自报身份。
- `nonce` 不是鉴权凭据：仅用于会话关联与重放检测；重放判决（同 nonce 二次出现即拒，拒绝码
  `bad_ticket`）由被访侧执行，台账容量与清理策略属实现配置。
- 访侧"能连上被访节点"不获得任何目标访问权：一切授权在被访侧门禁（§5.2）。

### 5.2 准入与审计

- 被访侧目标白名单 MUST 显式配置（`127.0.0.1:<port>` 精确匹配），默认空 = 全拒；MUST 支持
  "按次开启"：开启为会话态，默认关闭，不持久化，进程重启回落关闭。
- 访侧本地监听 MUST 只绑 `127.0.0.1` 字面量（§3.3）。
- 审计字段闭集（冻结）：`session_id / peer_id / target / started_at / ended_at / bytes_in /
  bytes_out / outcome`。口径：
  - `session_id` = 票据 `uid`（两侧日志同源）；`peer_id` = 底座握手 PeerId。
  - `started_at`/`ended_at` = Unix 秒；`ended_at` 在会话存续期为空。
  - `bytes_in`/`bytes_out` 按记录方视角：in = 自隧道收到，out = 向隧道发出；两侧各自落账。
  - `outcome` = `ok`（两侧 finish 正常关闭）或 §4 错误码闭集之一。
- 审计落盘格式与保留策略不属线格式（实现职责）；但每次会话 MUST 落一条终态记录，关闭原因
  MUST 体现于 `outcome`。

## 6. 兼容与版本

- 加法不破坏：票据/应答 JSON 的未知字段 MUST 忽略；`v` ≠ 1 一律 `bad_ticket`，不做降级协商。
- 不兼容变更 = 升协议 ID 版本（`/p2p-base/tunnel/2` 新建规范页，本页顶部标注 Superseded），
  禁止原地修改 v1 语义；票据帧上限、错误码闭集、超时值、准入字段的变更均属不兼容变更。
- 探测：开流写本协议 ID，未注册对端关流（UnsupportedProtocol，底座行为）即不支持。
- 本协议不设能力协商；responder 的白名单与并发上限差异不进线格式（表现为
  `target_not_allowed`/`busy` 拒绝）。

## 7. 测试向量

无。首波向量集冻结清单（spec-charter §8）未含本协议：帧封装字节语义由 frame.json 覆盖
（该向量集落地后直接复用，不重复登记）；票据帧/应答帧黄金 JSON 字节与 `ts` 边界（±300s）
列为后续向量集候选；门禁与半关闭行为由实现侧单元测试与回环用例覆盖（W-T2/W-T3 落地时补）。

## 8. 实现状态与出处

实现未合并：registry `impl = "planned"`（本页随 W-T1 契约先行冻结；被访侧实现 = W-T2，
访侧实现 = W-T3）。冻结接口清单（契约 §7）：

| 项 | 归属 | 形状 |
|---|---|---|
| `PROTOCOL_ID` | crates/p2p-tunnel | `/p2p-base/tunnel/1` 唯一定义点 |
| `TunnelTicket` | crates/p2p-tunnel | §2.2 字段表 |
| `TunnelErrorCode` | crates/p2p-tunnel | §4 六值闭集 |
| `TunnelResponder<H: HttpDialer>` | crates/p2p-tunnel | impl `ProtocolHandler`（被访侧受理） |
| `HttpDialer` | crates/p2p-tunnel | `async fn dial(&self, target: &str) -> Result<TunnelIo, TunnelError>` |
| `TunnelClient<S: StreamFactory>` | crates/p2p-tunnel | `open(peer, ticket) -> TunnelIo`（访侧） |
| `LocalProxy<C>` | crates/p2p-tunnel | `bind(127.0.0.1:0)` + 回传 `local_addr` |
| GUI 命令/事件 | apps/gui/src-tauri | `tunnel_open_dsh` / `tunnel_status` / `tunnel_serve_start` / `tunnel_serve_stop`，事件 `tunnel_status`（含 serve 字段；gui-contract §19） |

已知偏差与漂移登记：无（初版）。规范页与实现冲突时以代码为准并登记本节。
