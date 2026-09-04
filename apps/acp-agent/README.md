# acp-agent

ACP over P2P 的 agent 侧端点：监听 `/dsh-acp/1`，按策略表把远程 peer 桥接到
每连接一个的 ACP 子进程（默认 `pnpm dsh --profile acp`）。协议面与安全模型见
`docs/design/acp-over-p2p-design.md`；本文件只写**桥约定（bridge convention）**——
两端都是我们的代码，桥在透传层插入的私有语义全部登记在此，GUI 波按此对接。

## 桥约定契约（v1，ACP4 拍板）

### 续连票据

- fresh 握手的 ready 帧携带 `ticket`（加法字段，缺省不序列化）：
  `{"ready":{"scope":"sandbox","agent":"home-agent","bridge":"1","ticket":"<uuid>"}}`。
- 票据由桥签发，绑定签发时的 PeerId；**仅签发 peer 本人可携回重连**，跨设备持他人
  票据一律 `reattach-ticket-invalid` 拒绝并审计（防跨设备劫持）。
- 票据随槽位存活：子进程退出（窗口过期 / 崩溃 / 顶替）即失效。

### 续连流程

1. 客户端断流：桥不杀子进程，进入续连窗口（默认 90s，`--reattach-window-secs` 可配）。
2. 窗口内重连：握手行带 `"reattach":"<ticket>"`（ClientHello 既有字段），ready 回带原票据。
3. 窗口内 `session/update` 逐条入每会话环形缓存（8 MiB，超限丢最旧并留日志）；
   非 update 的子进程行（如 prompt 响应）不缓存、丢弃并留日志——prompt 的迟到结算
   由 GUI 侧按设计 §5 约定处理。
4. 窗口内 outstanding 的 `request_permission` 立即代答 reject-once（无人值守 = 拒绝）。
5. 重连后客户端发出 `initialize`，该请求过桥写入子进程后，桥先向 wire 补放：

       {"jsonrpc":"2.0","method":"dsh/bridge/reattach","params":{"replayed":N}}

   （桥约定通知，无 id，重放协议合法；GUI 据此显示“已续连，补放 N 条错过的更新”。）
   随后按会话名序、行序补放缓存行，之后恢复实时透传。
6. 窗口过期或子进程崩溃：走既有退出阶梯（stdin EOF -> 宽限 -> SIGKILL）。
7. 窗口内**不带票据**的新连接视为放弃续连：桥顶替（supersede）遗留槽位后按 fresh 流程新建。

### 权限瀑布（request_permission）

- 匹配规则：子进程行 `method` 以 `request_permission` 结尾且携带非空 id。
- 静态策略先行：`params.toolCall.kind` 为 `read` / `think` / `fetch` 时桥直接代答
  选中第一个 `kind` 以 `allow` 开头的选项：

       {"jsonrpc":"2.0","id":ID,"result":{"outcome":{"outcome":"selected","optionId":"<allow 选项>"}}}

  该请求不透传给客户端；客户端会看到一条"未经请求的"响应，GUI 需按策略提示已自动放行。
- 其余 kind（execute / edit / delete 及未知值，保守默认）进入 ask 路由：
  - `remote_gui`（默认）：透传客户端，桥登记 outstanding；客户端 `--permission-timeout-secs`
    （默认 60s）内未应答，桥代答 reject-once：

         {"jsonrpc":"2.0","id":ID,"result":{"outcome":{"outcome":"cancelled"}}}

  - `owner_local`（per-peer 策略）：请求不透传，桥本地审计并立即 reject-once 占位
    （交互面由 GUI 波接管）。
- grant 一次性：桥不持久化任何许可状态（allow_always 选项不桥侧记忆）。

### mcpServers 处置（session/new 安全改写点）

- 默认（`allow_mcp` 为空）：`params.mcpServers` **整字段剥离**后转发。
- 白名单 peer：数组每项必须是 `{"name":"<名>"}` 按名引用，且名字同时在
  `allow_mcp` 白名单与 node 配置 `mcp_definitions`（完整 host 侧定义，含命令字节）；
  桥把数组整体替换为对应 host 定义。命令字节永远在 host 手里。
- 违例（白名单外名字、非按名引用、host 未预定义）**整请求拒绝**：不转发子进程，
  桥回 JSON-RPC 错误 `{"code":-32602,"message":"mcp-servers-rejected"}`（带原 id）；
  notification（无 id）静默丢弃仅审计。
- 所有剥离 / 替换 / 拒绝动作留审计日志（`mcp-rewritten`）。

### cwd 监狱

- `sandbox`（默认）：子进程 cwd = `<sandbox_root>/<peerId>/`，目录不存在则创建；
  目录名经字符白名单净化。`--sandbox-root` 缺省为 `<data_dir>/sandbox`。
- `workspace`：锁定 `--workspace-dir` 配置目录（symlink 解析到真实目标）；
  未配置则拒绝连接（wire 码 `cwd-denied` + 审计）。
- `owner`：继承桥 cwd（全 root）。**仅限本机 loopback 场景授予**，远程 peer 授予
  owner 等同交出整机，属操作者责任（TOFU + 指纹确认面拦截）。

### 桥自身退出

- 收到 SIGTERM/Ctrl-C：先停监听，再向全部活槽位广播 Shutdown，各子进程按
  退出阶梯收尾（stdin EOF -> 宽限 -> SIGKILL），限时等待、超时留 error 日志
  并由 kill_on_drop 兜底。幂等可重复调用。

### 其他桥约定

- 客户端行的 `session/new` 之外的字节零改动透传；子进程行除上述两个安全改写点外
  零改动透传（协议智能不进 Rust 的边界不变）。
- 凭据只在子进程环境；wire 上只有语义更新。
- 审计事件（默认落 `<data_dir>/acp-agent.log`，target `acp_audit`）：
  conn-denied / gate-denied / conn-established / spawn-failed / client-gone /
  subprocess-exit / cwd-denied / mcp-rewritten / permission-acted /
  reattach-accepted / reattach-denied / window-expired / slot-superseded。

## 自测

    cd apps/acp-agent
    cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings

## 设计索引

- 断线续连：design §5；安全模型：design §6；生命周期与门禁：design §7；
  拍板记录：design §12（Q1 窗口 90s / Q2 默认 sandbox / Q3 ask 默认路由远程 GUI）。