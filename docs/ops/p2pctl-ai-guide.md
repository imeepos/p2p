# p2pctl AI 接入指南（自描述工具面）

面向 AI/LLM 的 p2pctl 自描述工具面：任意大模型读完本文即可驱动 p2p 全部命令能力。
本文与实现机械同步——`scripts/check/ai-docs-sync.sh`（`make check` 门禁）递归解析
p2pctl 实测 `--help` 命令面，逐条断言本文含该命令条目、参数名与实现逐字一致；
文档含实现不存在的命令即红。**AI 与人都不手改命令目录**，发现实现缺陷回报协调者。

- 二进制构建：`cargo build --manifest-path apps/cli/Cargo.toml`，产物 `apps/cli/target/debug/p2pctl`（下文简称 `p2pctl`）。
- 本文档示例全部实测采样；`<data-dir>` 等尖括号为占位符，实际值见各命令 `--data-dir` 默认值。

## 1. 全局约定

### 1.1 退出码（除条目特别标注外统一适用）

| 退出码 | 含义 |
|---|---|
| 0 | 成功（含幂等"无事可做"：重复 stop、remove 不在簿好友、clear 已空日志） |
| 1 | 运行失败（节点未运行、白名单拒绝、校验失败、超时未送达等）；stderr 前缀 `p2pctl: 运行失败: `，退出码与错误信号可观测 |
| 2 | 用法错误（参数缺失/非法，clap 报告，stderr 前缀 `error: `） |

### 1.2 输出双形态

读命令默认输出人读文本（`key=value` 行为主，供 grep 无依赖采集）；加 `--json`
输出同源结构化 JSON（camelCase，与 GUI 契约字段同形）。写命令同约定。

### 1.3 数据目录与两套身份根

- 全局参数 `--data-dir`（默认 `./p2p-data`）存放 CLI/GUI 共享数据（`gui-config.json`、
  `node-profile.json`、聊天库 `chat/`）。
- **身份有两套根**：chat 域身份与聊天库同根（`<data-dir>/chat` 一侧，`chat serve`
  输出的 peerId 即它）；节点守护身份取配置 `dataDir`（缺省回落
  `<data-dir>/p2p-data`）。因此 `chat serve` 与 `node start` 输出的 peerId 可以不同，
  属正常现象。
- 守护进程可观测信号：`daemon.pid` / `daemon.meta.json` / `daemon.sock` / `daemon.log`。

### 1.4 前置条件矩阵

| 前置 | 适用命令 | 不满足时的表现 |
|---|---|---|
| 无（离线可跑） | config、profile、chat friends/history/media、chat serve、identity reset、log tail/path/clear、metrics get、update check/open、node status | —— |
| 对端在线可达 | chat send（真正送达）、peer dial/connect/ping | chat send 退出 1，消息保留本机 status=Pending；peer 域退出 1 |
| 节点守护进程运行 | peer connect/disconnect/ping/dial、metrics get 实时值 | 退出 1：连接节点守护进程失败；metrics get 例外：返回全零不报错 |
| GUI 进程运行 | gui 全域（status/screenshot/record/navigate/invoke/page/action） | 退出 1（控制通道不可达） |
| GUI 日志目录存在（默认自动） | log tail/clear | 退出 1，文件不存在类错误 |

## 2. AI 意图 → 命令映射表

| 意图 | 命令 |
|---|---|
| 发消息 | `p2pctl chat send --peer <PEER_ID> --text "..." --json` |
| 查好友 / 加好友 / 删好友 | `chat friends list` / `chat friends add <PEER_ID>` / `chat friends remove <PEER_ID>` |
| 看消息历史 | `chat history --peer <PEER_ID> --json` |
| 查附件落盘路径 | `chat media file --peer <PEER_ID> --message-id <ID> --json` |
| 看节点状态 | `p2pctl node status --json` |
| 启动 / 停止节点 | `node start` / `node stop` |
| 测连通 / 拨号 / 挂断 | `peer ping <PEER_ID>` / `peer dial "<PEER_ID>@<ADDR>"` / `peer disconnect <PEER_ID>` |
| 查 / 改配置 | `config get --json` / `config save -`（写，须人确认） |
| 查 / 改节点资料 | `profile get --json` / `profile save -`（写，须人确认） |
| 查运行时指标 | `metrics get --json` |
| 截 GUI 图 | `gui screenshot --output /abs/path.png --json` |
| 切 GUI 页面 | `gui navigate <ROUTE> --json`（dashboard/peers/discovery/relay/chat/events/settings/diagnostics） |
| GUI 白名单只读转发 | `gui invoke metrics_get --json` |
| 看当前页能做什么 | `gui page --json`（actions 与参数 schema，写动作带 [confirm] 标记） |
| 执行页面动作 | `gui action <页面> <动作> K=V... --json`（非当前页加 `--navigate`；写类动作须人确认） |
| 查前端日志 | `log tail --json` / `log path --json` |
| 清前端日志 | `log clear`（写，须人确认） |
| 查新版本 | `update check --json` |
| 重置身份（红线） | `identity reset`——不可逆，见 §3 |

## 3. 开场提示词模板（整段贴给 LLM 即可）

```text
你是 p2p 节点的运维助手，通过 p2pctl 命令行工具操作 p2p。请严格遵守：

【工具认知】
- 可执行文件：apps/cli/target/debug/p2pctl（先 cargo build --manifest-path apps/cli/Cargo.toml 构建若不存在）。
- 命令面：node|chat|config|profile|peer|gui|identity|log|metrics|update 十域，共 33 个叶子命令。
- 每个命令先跑 --help 确认参数，再执行；官方命令参考见 docs/ops/p2pctl-ai-guide.md。
- 输出：默认人读文本（key=value 行），加 --json 得结构化 JSON（camelCase）。
- 退出码：0 成功；1 运行失败（stderr 前缀 "p2pctl: 运行失败: "）；2 用法错误。失败时先读 stderr 再决定下一步，不要盲目重试。

【安全边界（最高优先级）】
1. 只读命令优先：node status / chat friends list / chat history / config get / metrics get /
   log tail / gui status / update check 等查询类命令可自由执行。
2. 写操作必须先征得人确认再执行：chat send / chat friends add|remove / config save /
   profile save / node start|stop / peer dial|connect|disconnect / log clear / gui navigate。
3. 不可逆红线：identity reset 会删除节点身份（key.seed），除非人明确说"重置身份"，
   永远不得执行；执行时必须带 --confirm 且仅限人指定的数据目录。
4. 不得绕过安全机制：gui navigate/gui invoke 仅接受服务端白名单（8 个路由、5 个只读命令），
   不得尝试注入白名单外命令或伪造令牌；不得猜测/读取他人 peer id 之外的凭据。
5. 数据目录隔离：演示与实验一律用 --data-dir 指向临时目录，禁止把测试数据写进正式目录。

【工作方式】
- 先跑 p2pctl node status --json 判断节点是否在线，再决定是否需要人确认启动节点。
- chat send 前先用 chat friends list 确认对端在簿且在线；发送超时未送达（退出码 1，
  status=Pending）时如实报告，不要伪造"已送达"。
- 每一步给出：执行的命令、退出码、关键输出摘要；失败时附 stderr 原文。
```

## 4. 安全边界细则（人读版）

- **只读优先**：AI 的默认动作集合是 §1.4"无前置"表中的读命令与 §2 查询类意图。
- **写须确认**：所有改变状态的动作（发消息、改配置/资料、增删好友、启停节点、清理日志、
  切 GUI 页面、gui action 写类页面动作如 addFriend/saveConfig）执行前必须向人复述将执行的确切命令并获得同意。
- **identity reset 红线**：删除 key.seed 不可恢复；缺 `--confirm` 时实现层即拒绝
  （退出 1），这是最后防线而非授权，AI 不得在任何未经人明确同意的情况下携带 `--confirm`。
- **token/白名单不可绕过**：gui 域命令经控制通道由 GUI 服务端白名单校验（路由 8 个、
  invoke 只读命令 5 个），被拒绝即为终态，AI 不得重试变形绕过；身份凭据（key.seed）
  只能由实现读取，AI 不得打印、复制或迁移其内容。

## 5. 与 GUI 的关系

p2pctl 是 GUI（p2p-console，Tauri 应用）命令面的等价 CLI，由 `scripts/check/cli-parity.sh`
守卫对等；`gui` 域是控制 GUI 本身的原语（需 GUI 进程运行），其余域与 GUI 各页面读写
同一份数据。GUI 数据目录（macOS）`~/Library/Application Support/com.p2p.console`，前端
日志 `~/Library/Logs/com.p2p.console/frontend.log`，均可用 `--gui-data-dir`/`--log-dir` 覆盖。

## 6. 命令面全目录（33 命令）

条目格式：用途/前置 → 参数表（名称/类型/必填/默认）→ 文本输出例 → --json 输出例。
类型取值：flag（无值开关）/string/int/path/kv/枚举值说明。尖括号示例为实测采样占位。

<!-- AI-DOCS-SYNC:BEGIN（机器校验区间，禁手改结构） -->

### p2pctl node status
用途：查询本机节点运行状态；未运行也退出 0（running=false）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
节点未运行（无 pid 文件 <data-dir>/daemon.pid）
```
--json：
```
{"running":false,"pid":null,"logPath":"<data-dir>/daemon.log","dataDir":"<data-dir>","degraded":false,"reason":"无 pid 文件 <data-dir>/daemon.pid"}
```

### p2pctl node start
用途：启动节点守护进程（读 gui-config.json，缺省用默认配置）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
节点已启动 pid=80955
pid=80955
```
--json：
```
{"running":true,"alreadyRunning":false,"pid":80955,"peerId":"<PEER_ID>","listenAddrs":["127.0.0.1/u64935","127.0.0.1/t62038"],"uptimeSecs":0,"logPath":"<data-dir>/daemon.log","dataDir":"<data-dir>","degraded":false,"reason":""}
```
退出码：已运行再 start 返回 0（alreadyRunning=true）。

### p2pctl node stop
用途：停止节点守护进程，幂等。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已停止节点（pid=80955）
```
--json：
```
{"stopped":true,"pid":80955}
```
退出码：未运行时重复 stop 报"未运行"，仍退出 0。

### p2pctl chat friends list
用途：列出全部好友。前置：无（空簿输出"好友簿为空"）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
好友簿为空
```
--json（单行紧凑）：
```
[{"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj","nickname":"Bob","addrs":[],"note":null}]
```

### p2pctl chat friends add
用途：添加好友（幂等 upsert，已在簿则为更新并 created=false）。前置：无；PEER_ID 不得是本机 chat 身份。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --nickname | string | 否 | "" |
| --addr | string（可重复） | 否 | 无 |
| --note | string | 否 | 无 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已添加好友 Alice（54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4）
```
--json：
```
{"created":true,"friend":{"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj","nickname":"Bob","addrs":[],"note":null}}
```
退出码：PEER_ID 非 32 字节 base58 → 1（PeerId 非法）；PEER_ID 为本机 chat 身份 → 1（不能与自己通信）。

### p2pctl chat friends remove
用途：移除好友，幂等。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已移除好友 54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4
```
--json（不在簿时）：
```
{"removed":false}
```

### p2pctl chat history
用途：读与某对端的消息历史（时间倒序）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --peer | string | 是 | —— |
| --before-id | string | 否 | 无 |
| --limit | int | 否 | 50（上限 100） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
无历史消息（peer=HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj）
```
--json：
```
[]
```

### p2pctl chat send
用途：发送消息（--text 文本或 --file 附件，二选一）。前置：对端在线可达才真正送达；PEER_ID 不得是本机 chat 身份。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --peer | string | 是 | —— |
| --text | string | 与 --file 二选一 | —— |
| --file | path | 与 --text 二选一 | —— |
| --kind | text/image/audio/video/file | 否 | 按载荷推断 |
| --mime | string | 否 | 按扩展名推断 |
| --name | string | 否 | 取文件名 |
| --reply-to | string | 否 | 无 |
| --timeout-secs | int | 否 | 30 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（对端不可达，超时后）：
```
未送达 peer=54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4 id=425a11cd-4b71-4397-ba6e-4771e1414a0d status=Pending
```
stderr：
```
p2pctl: 运行失败: 消息未送达对端（status=Pending），已保留本机记录
```
退出码：超时未送达 → 1（消息保留本机 status=Pending，可经 history 复核）；PEER_ID 为本机身份 → 1。

### p2pctl chat media file
用途：查询附件落盘绝对路径。前置：无（消息 id 必须存在于本机 history）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --peer | string | 是 | —— |
| --message-id | string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（消息不存在时）：
```
（无 stdout，错误走 stderr）
```
stderr：
```
p2pctl: 运行失败: 未找到：消息不存在：abc
```
退出码：消息不存在 → 1；存在 → 0（输出绝对路径行 / JSON absolutePath）。

### p2pctl chat serve
用途：常驻运行聊天节点，输出 peerId 与监听地址后等待信号（E2E/守护支撑）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --data-dir | path | 否 | ./p2p-data |
| --quic-port | int | 否 | 随机 |
| --mdns | flag | 否 | off |
| --json | flag | 否 | off |
文本：
```
chat 节点就绪 peer=7D5SoJj1Cm1pN4xwrwVEeUCWgwq4hr9MaHcfcBUvrtyz listen=127.0.0.1/u60645 127.0.0.1/t62230
```
--json：
```
{"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj","listenAddrs":["127.0.0.1/u59286","127.0.0.1/t61793"],"dataDir":"<data-dir>"}
```
注意：常驻命令，AI 应以人确认后使用，且用完发 SIGINT/SIGTERM 退出。

### p2pctl config get
用途：读取持久化配置；无文件输出 GUI 出厂默认值。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（key=value 行）：
```
quicPort=0
tcpPort=0
enableMdns=true
dataDir=<data-dir>/p2p-data
bootstrap=43.240.223.138/u3400,121.196.193.177/u3400
relayAddrs=43.240.223.138/u3403,121.196.193.177/u3403
```
--json（节选）：
```
{"quicPort":0,"tcpPort":0,"enableMdns":true,"dataDir":"<data-dir>/p2p-data","bootstrap":["43.240.223.138/u3400"],"relayAddrs":["43.240.223.138/u3403"],"advertisedAddrs":[],"observationPort":null,"observationAddrs":["121.196.193.177:3402"]}
```

### p2pctl config save
用途：保存完整配置 JSON（覆盖式）。前置：无；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| [CONFIG] | 位置参数 string | 否 | 省略或 "-" 读 stdin |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（成功后回显保存值，key=value）：
```
quicPort=0
tcpPort=0
enableMdns=true
```
--json：
```
{"quicPort":0,"tcpPort":0,"enableMdns":true,"dataDir":"<data-dir>/p2p-data","bootstrap":["43.240.223.138/u3400"],"relayAddrs":["43.240.223.138/u3403"],"advertisedAddrs":[],"observationPort":null,"observationAddrs":["121.196.193.177:3402"]}
```
退出码：JSON 解析失败 → 1。推荐管道：`p2pctl config get --json | p2pctl config save - --json`。

### p2pctl profile get
用途：读取节点资料；无文件输出默认值。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
name=n
description=
avatar=未设置
```
--json：
```
{"name":"n","description":"","avatar":null}
```

### p2pctl profile save
用途：保存节点资料 JSON。前置：无；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| [PROFILE] | 位置参数 string | 否 | 省略或 "-" 读 stdin |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
--json（回显保存值）：
```
{"name":"n","description":"","avatar":null}
```
退出码：JSON 解析失败 → 1。

### p2pctl peer dial
用途：拨号 "<peer_id>@<addr>"（addr 为 ip/u端口 或 ip/t端口）。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <TARGET> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
stderr（守护未运行）：
```
p2pctl: 运行失败: 连接节点守护进程失败: No such file or directory (os error 2)
```
退出码：守护不可达 → 1。

### p2pctl peer connect
用途：按地址簿连接已知节点。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
stderr（守护未运行）：
```
p2pctl: 运行失败: 连接节点守护进程失败: No such file or directory (os error 2)
```

### p2pctl peer disconnect
用途：挂断与该节点的连接，幂等。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
stderr（守护未运行）：
```
p2pctl: 运行失败: 连接节点守护进程失败: No such file or directory (os error 2)
```

### p2pctl peer ping
用途：echo 协议测 RTT。前置：节点守护进程运行且对端可达。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --timeout-ms | int | 否 | 5000 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（成功）：`rtt_ms=<数值>` 形态 key=value 行。
stderr（守护未运行）：
```
p2pctl: 运行失败: 连接节点守护进程失败: No such file or directory (os error 2)
```

### p2pctl gui status
用途：查询运行中 GUI 的状态（版本/窗口/当前路由）。前置：GUI 进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本：
```
version=0.1.2
window=p2p-console
route=chat
pid=48436
uptimeMs=1214604
recording=false
```
--json：同字段 camelCase（version/window/route/pid/uptimeMs/recording）。

### p2pctl gui screenshot
用途：截图主窗口内容并落盘 PNG。前置：GUI 进程运行；输出路径须绝对。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| -o, --output | path | 是 | —— |
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本：
```
path=/tmp/p2p-ai-probe/s2.png
width=1080
height=800
bytes=163208
```
--json：
```
{"bytes":128320,"height":800,"path":"/tmp/p2p-ai-probe/shot.png","width":1080}
```

### p2pctl gui record start
用途：开始录屏（产物 GIF）。前置：GUI 进程运行；输出路径须绝对。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| -o, --output | path | 是 | —— |
| --interval-ms | int（200..5000） | 否 | 500 |
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本：
```
path=/tmp/p2p-ai-probe/r2.gif
intervalMs=500
```

### p2pctl gui record stop
用途：停止录屏并等待产物落盘。前置：先 record start。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本：
```
path=/tmp/p2p-ai-probe/r2.gif
frames=2
bytes=36824
truncated=false
```
--json：
```
{"bytes":29441,"frames":2,"path":"/tmp/p2p-ai-probe/rec.gif","truncated":false}
```

### p2pctl gui navigate
用途：按路由名切换 GUI 页面。前置：GUI 进程运行；路由白名单 8 个。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <ROUTE> | 位置参数枚举：dashboard/peers/discovery/relay/chat/events/settings/diagnostics | 是 | —— |
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本：
```
route=chat
path=#/chat
```
--json：
```
{"path":"#/chat","route":"chat"}
```
退出码：白名单外路由 → 1，stderr 含 [INVALID_ROUTE] 与可用路由表（不得绕过白名单）。

### p2pctl gui invoke
用途：转发白名单内只读 GUI 命令（node_status/metrics_get/metrics_history/config_get/profile_get）。前置：GUI 进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <COMMAND> | 位置参数枚举（5 个白名单命令） | 是 | —— |
| --arg | kv（k=v，可重复） | 否 | 无 |
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
--json（metrics_get）：
```
{"result":{"activeConnections":1,"addrDialFailures":0,"dialDirectFail":0,"dialDirectOk":1,"dialPunchFail":0,"dialPunchOk":0,"dialRelayFail":0,"dialRelayOk":0,"gateDenialsTotal":0,"relayReconnects":0,"relaySessionsActive":2}}
```
退出码：白名单外命令 → 1（不得绕过）。

### p2pctl gui page
用途：查询当前页语义 descriptor：name/description 与 actions 表格（动作与页面按钮同源走 store/IPC，非 DOM 模拟）。前置：GUI 进程运行；当前页未在注册表（如 dashboard）时服务端 PAGE_NOT_REGISTERED 拒绝。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本（每动作一行，args 标注「类型,必填性」，危险动作带 [confirm] 标记）：
```
name=chat
description=IM 聊天页：好友会话文本发送与好友管理
actions=3
- sendText: 向好友发送文本（乐观更新，与聊天输入框同源）
  args: peer(string,必填) text(string,必填)
- removeFriend: 移除好友（不删本地消息历史），与移除确认框同源 [confirm]
  args: peer(string,必填) confirm(boolean,必填)
```
--json：服务端全量 descriptor（含 args schema、state 快照与 schemaVersion）。
退出码：当前页未注册 → 1（PAGE_NOT_REGISTERED，错误信息含可用页清单）。

### p2pctl gui action
用途：执行页面动作（与页面按钮同源）。前置：GUI 进程运行；非当前页须先 gui navigate <页面> 或携带 --navigate。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PAGE> | 位置参数 string（已注册页名） | 是 | —— |
| <ACTION> | 位置参数 string（注册表动作名，见 gui page 输出） | 是 | —— |
| [ARGS] | 位置参数 kv（k=v 可重复；布尔/数字按 JSON 类型解析，与 invoke 域同规则） | 否 | 无 |
| --navigate | flag | 否 | off |
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本（动作返回值的人读 JSON，chat addFriend 例）：
```
{
  "peerId": "11111111111111111111111111111111111111111111",
  "nickname": "gc4-e2e",
  "addrs": [],
  "note": null
}
```
--json：同源结构化（data 即动作返回值原样）。
退出码：非当前页 → 1（结构化错误含「gui navigate <页面>」指引）；危险动作缺 confirm=true → 1（ACTION_CONFIRM_REQUIRED 透传）；动作不存在 → 1（ACTION_NOT_FOUND，含可用动作清单）。

### p2pctl identity reset
用途：重置身份：停节点 + 删除 key.seed，不可逆。前置：无；红线见 §4。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --confirm | flag | 是（危险确认） | off |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（确认后）：
```
身份已重置（stoppedNode=false）
seed=<data-dir>/p2p-data/key.seed
```
stderr（缺 --confirm）：
```
p2pctl: 运行失败: 重置身份是危险操作，必须显式传入 --confirm
```
退出码：缺 --confirm → 1（拒绝执行）；确认后 → 0 且不可恢复。

### p2pctl log tail
用途：读 GUI 前端日志末尾 N 行（frontend.log，JSONL）。前置：日志目录存在（默认自动）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --log-dir | path | 否 | GUI 日志目录 |
| --lines | int | 否 | 200（上限 1000） |
文本：逐行原样输出日志 JSONL 行。
```
{"t":"sample"}
```
--json：
```
{"path":"<log-dir>/frontend.log","lines":[{"level":"info","message":"..."}]}
```

### p2pctl log path
用途：输出 GUI 前端日志文件绝对路径。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --log-dir | path | 否 | GUI 日志目录 |
文本：
```
/Users/imeepos/Library/Logs/com.p2p.console/frontend.log
```
--json：`{"path":"<log-dir>/frontend.log"}` 形态。

### p2pctl log clear
用途：清理 GUI 前端日志（连轮转代 frontend.log.1 一起删），幂等。前置：日志目录存在；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --log-dir | path | 否 | GUI 日志目录 |
文本：
```
已清理前端日志 current=true rotated=false path=<log-dir>/frontend.log
```
退出码：已空再 clear 仍退出 0。

### p2pctl metrics get
用途：读取运行时指标快照。前置：无（未运行返回全零，不报错）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
--json：
```
{"dialDirectOk":0,"dialDirectFail":0,"dialPunchOk":0,"dialPunchFail":0,"dialRelayOk":0,"dialRelayFail":0,"addrDialFailures":0,"relayReconnects":0,"gateDenialsTotal":0,"activeConnections":0,"relaySessionsActive":0}
```
文本：同字段 key=value 行。

### p2pctl update check
用途：检查 GitHub 最新稳定 release 并与当前版本比较。前置：网络可达 GitHub（10s 超时）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
文本：
```
current=0.1.0
latest=client-v0.1.2
hasUpdate=true
url=https://github.com/imeepos/p2p/releases/tag/client-v0.1.2
name=client-v0.1.2
```
--json：同字段 camelCase（current/latest/hasUpdate/url/name）。

### p2pctl update open
用途：输出 release 页 URL（CLI 不开浏览器）。前置：缺省 --url 时先经网络检查。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --url | string | 否 | 先检查取最新候选 |
| --json | flag | 否 | off |
文本：
```
https://github.com/imeepos/p2p/releases/tag/client-v0.1.2
```
--json：
```
{"url":"https://github.com/imeepos/p2p/releases/tag/client-v0.1.2"}
```
退出码：--url 非 https/github.com 白名单 → 1。

<!-- AI-DOCS-SYNC:END -->
