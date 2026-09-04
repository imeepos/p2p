# acp-console

ACP over P2P 的操作者侧伴生进程（设计 docs/design/acp-over-p2p-design.md §3）：
本地 WS ⇄ P2P 流的哑泵 + 节点发现 + 连接状态机。GUI（ACP6a/6b 波）作为标准
WS 客户端接入本进程；本进程不解析 ACP 语义，wire 一个字节不改。

本卡（ACP3）只交付骨架：拨号+握手、本地 WS 服务、状态机、发现清单、回环测试。
GUI 渲染与续连逻辑分别在 ACP6a/6b 与 ACP4。

## 运行

```
cargo run -p acp-console -- \
  --bootstrap 192.168.1.10/u7001 \        # 可选，可多次
  --peer <base58PeerId>@/ip4/10.0.0.8/tcp/4001 \  # 可选，可多次
  --ws-port 0 --status-port 0              # 0 = 随机端口
```

启动成功即向 stdout 打一行就绪事件（见下），GUI/CLI 从 stdout 读端口与 token。

## stdout JSON 行契约（CLI 可读）

每行一个 JSON 对象，`kind` 区分事件：

| kind | 载荷 | 说明 |
|---|---|---|
| `ready` | `{ws, status, token, peer}` | 就绪。ws/status 为 `127.0.0.1:port`；token 为本进程鉴权 token；peer 为自身 PeerId |
| `state` | `{phase, peer?, conn?, since_unix_ms, detail?}` | 连接状态机每次迁移 |
| `discovery` | `{peers: [{peer, addrs, source}]}` | 发现清单每次变更，全量快照 |

## 本地 WS 契约（GUI 波依赖）

```
ws://127.0.0.1:<ws_port>/?token=<token>&peer=<base58PeerId>[&reattach=<uuid>][&atoken=<agent-token>]
```

- 鉴权：`token` 必填且精确匹配；绑定 127.0.0.1 + token 双条件（设计 §6 防 drive-by）。
  无 token / 错 token 在 HTTP 升级层以 **401** 拒绝并落审计日志（只记长度，不记材质）。
- `peer` 必填：目标 agent 节点 PeerId。console 向其拨 `/dsh-acp/1` 流并交换握手帧
  （conn=随机 uuid，`atoken` 可选透传，`reattach` 可选透传给 ACP4）。
- 握手 `ready` → 进入 online，此后 **纯字节泵**：WS Binary/Text 帧 ⇄ P2P 流按原始
  字节双向透传（WS 读侧单消息上限 16 MiB，对齐 acp-common 单行护栏）。
- 关闭语义（双向传播）：
  - agent 断流 → WS 下发 Close(1000)；
  - WS 客户端 Close → P2P 流写半 EOF；
  - agent 拒绝握手 → WS Close(**4403**, `denied:<code>`)；
  - 拨号/握手失败 → WS Close(**4500**, `dial-failed`)。

## status 端点契约（查询方式拍板：本地 HTTP，GUI 轮询用）

```
GET http://127.0.0.1:<status_port>/status     Authorization: Bearer <token>
GET http://127.0.0.1:<status_port>/discovery  Authorization: Bearer <token>
```

- `/status` → 连接状态机快照 JSON：`{phase, peer?, conn?, since_unix_ms, detail?}`。
- `/discovery` → `{"peers":[{peer, addrs, source}]}`，与 stdout discovery 行同形状。
- 无 token / 错 token → **401** + 日志。

## 连接状态机

`offline → connecting → online → reattach-window → offline`

| 迁移 | 触发 |
|---|---|
| connecting | WS 连接鉴权通过，开始拨号 |
| online | 握手收到 ready（票据 conn+peer 落盘） |
| reattach-window | 泵结束（任一侧断流）；窗口默认 90s（`--window-secs`） |
| offline | 拨号/握手失败；或窗口到期未被新连接接管 |

每次迁移：tracing 日志 + stdout `state` 行 + status 快照更新。

## reattach 票据（ACP4 续连入口）

`<data-dir>/reattach-tickets.json`（tmp+rename 原子写，损坏显式报错不静默清空）：

```json
{"version":1,"tickets":[{"conn":"<uuid>","peer":"<base58>","saved_at_unix_ms":0}]}
```

每 peer 留最新一条，总量截断 8 条。存取 API：`ticket::TicketStore::{save,latest,latest_for}`。

## 自测

```
cd apps/acp-console && cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings
```

单机回环测试（`tests/`）：in-test 起 facade 服务端模拟 agent（acp-common 握手帧应答），
覆盖拨号+握手 roundtrip、WS token 鉴权拒绝、WS⇄P2P 字节透传 roundtrip、
对端断流→offline、票据落盘与读取。
