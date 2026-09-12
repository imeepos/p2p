# 冻结契约 `/p2p-base/tunnel/1`（W-T1/W-T2/W-T3 共用，逐字下发）

本契约由协调者（主会话）冻结，三卡不得各自改义。W-T1 负责把它写成规范页与登记行；若实现中发现契约不可行，
**先报 BLOCKED 说明冲突**，由协调者裁决后再改，禁止单方面改义。

## 1. 协议 ID 与帧序

- 协议 ID：`/p2p-base/tunnel/1`（常量唯一定义点建议放新 crate `p2p-tunnel`）。
- 帧序：**首帧 = 票据帧（JSON UTF-8，≤4 KiB）→ 一帧应答（`ack` 或 `error`）→ 双向字节流分块**。
  首帧之后全部走底座帧封装（`p2p_protocol::{read_frame, write_frame}`，单帧上限 1 MiB）。
- 首帧超时：建流后 **5s** 内未收到合法票据即拒（`bad_ticket`）并关流。
- **分块合并约定**：出站 `write_chunked` 每次载荷 ≤ **64 KiB**（防高频小写入放大成帧风暴）；
  读侧必须容忍任意 ≤1 MiB 的分块边界（不得假设与写侧同粒度）。

## 2. 票据帧字段（`TunnelTicket`）

| 字段 | 类型 | 语义 |
|---|---|---|
| `v` | u8 | 固定 `1`；未知版本即 `bad_ticket` |
| `uid` | string | 本次访问会话 id（16 hex，访问侧生成，两侧日志同源） |
| `target` | string | 形态 `127.0.0.1:<port>`（**只允许回环字面量 `127.0.0.1`**；其他 host 一律 `target_not_allowed`） |
| `nonce` | string | 32 hex 随机（仅作会话关联与重放检测，**不是鉴权凭据**——鉴权靠底座握手 PeerId） |
| `ts` | u64 | 访问侧 unix 秒；被访侧窗口 ±300s，越界即 `bad_ticket` |

身份来自底座握手得到的 PeerId，**票据内不得自报身份，也不得采信任何自报身份**。

## 3. 应答帧

- `{"k":"ack","uid":"<同请求>"}`
- `{"k":"error","code":"<code>","msg":"<可选人读说明>"}`
- 错误码闭集（四处必须一致）：`bad_ticket` / `target_not_allowed` / `busy` / `dial_failed` / `io` / `shutdown`
- 拒绝路径必须显式回 `error` 帧，禁止静默断流。

## 4. 数据面语义

- 应答 `ack` 后进入字节流阶段，两个方向各自独立流动（HTTP 请求体与响应体互不阻塞）。
- `finish()` = 本方向半关闭；两侧均 finish 后整条隧道关闭。
- 任一侧 IO 错误/超时 → 整隧道关闭；关闭原因必须落审计（含 code）。
- WebSocket 升级：访问侧本地反代完成 HTTP 101 后，**升级后的 socket 与该流做裸字节双向泵**，不再有 HTTP 语义。

## 5. 准入与审计（安全硬约束）

- 被访侧：目标白名单是**显式配置**（`127.0.0.1:<port>` 精确匹配），默认空 = 全部拒绝；**按次开启**（会话态，默认关闭），
  未开启即 `target_not_allowed`。
- 访侧本地监听**只绑 `127.0.0.1` 字面量**（不用 `localhost`：cookie 只看 host 不看 port，必须与重写后的 Host authority 同源）。
- 会话审计：`session_id / peer_id / target / started_at / ended_at / bytes_in / bytes_out / outcome`；
  **不得记录**票据 nonce 以外的任何凭据（本次没有凭据，token 在 DSH cookie 层，不经过隧道协议）。

## 6. 访侧本地反代行为（W-T3）

- 收到浏览器请求后：**重写 `Host` 头为目标 `127.0.0.1:<port>`**，其余 method/path/headers/body 原样过隧道。
- 响应与 body **流式转发**（chunked），禁止整包缓冲；`Content-Length`/`Transfer-Encoding`/`Connection` 等
  hop-by-hop 语义按 HTTP/1.1 直通处理并明确记录取舍。
- DSH 启动 URL 解析：入参形如 `http://127.0.0.1:<port>/?token=<32+字节 base64url>`；
  输出 `local_addr` 与 `open_url = http://127.0.0.1:<local_port>/?token=<同 token>`（浏览器打开它触发 DSH 的 303 换 cookie）。

## 7. 冻结接口（供并行实现对齐，命名可微调但语义不得变）

- `p2p-tunnel`（新 crate）导出：
  - `pub const PROTOCOL_ID: &str = "/p2p-base/tunnel/1";`
  - `pub struct TunnelTicket { v, uid, target, nonce, ts }` + `encode()/decode()`
  - `pub enum TunnelErrorCode { BadTicket, TargetNotAllowed, Busy, DialFailed, Io, Shutdown }`（wire 字面量见 §3）
  - `pub struct TunnelResponder<H: HttpDialer> { … }` 实现 `ProtocolHandler`（被访侧）
  - `pub trait HttpDialer { async fn dial(&self, target: &str) -> Result<TunnelIo, TunnelError>; }`（测试可注入 fake）
  - `pub struct TunnelClient<S: StreamFactory> { … }`：`open(peer, ticket) -> TunnelIo`（访侧）
  - `pub struct LocalProxy<C> { … }`：`bind(127.0.0.1:0)` + 句柄回传 `local_addr`
- `apps/gui/src-tauri` 导出（W-T3 消费面）：
  - 命令 `tunnel_open_dsh(url: String) -> TunnelOpenResult { local_addr, open_url, token }`
  - 命令 `tunnel_status() -> TunnelStatus`（被访侧是否开启、白名单、活动会话数）
- Tauri 事件名：`tunnel_status`（变更时推送）。W-T1 负责把命令与事件登记进 `docs/design/gui-contract.md` 新章节（加法）。
