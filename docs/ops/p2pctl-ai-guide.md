# p2pctl AI 接入指南（自描述工具面）

面向 AI/LLM 的 p2pctl 自描述工具面：任意大模型读完本文即可驱动 p2p 全部命令能力。
本文与实现机械同步——`scripts/check/ai-docs-sync.sh`（`make check` 门禁）递归解析
p2pctl 实测 `--help` 命令面，逐条断言本文含该命令条目、参数名与实现逐字一致；
文档含实现不存在的命令即红。**AI 与人都不手改命令目录**，发现实现缺陷回报协调者。

- 二进制构建：`export PATH=$HOME/.cargo/bin:$PATH && cargo build --manifest-path apps/cli/Cargo.toml`，产物 `apps/cli/target/debug/p2pctl`（下文简称 `p2pctl`）。cargo 不在 PATH 时首跑报 `command not found`（2026-09-04 AI 试运行摩擦 F1），见 §1.4 前置矩阵。
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
  属正常现象。聊天收发（friends add / send、history --peer）一律使用 **chat 身份**
  peerId 与 `chat serve` 的监听地址；把守护 peerId 当聊天对端是最常见错法，表现为
  `chat send` 立即 status=Failed（见 §6 chat send 条目与附录A 配方）。
- 守护进程可观测信号：`daemon.pid` / `daemon.meta.json` / `daemon.sock` / `daemon.log`。
- **chat 身份互斥锁**：`<data-dir>/identity.lock`——chat 域进程同数据目录互斥：
  `chat serve` 常驻持锁期间，同 data-dir `chat send` 立即退出 1：`p2pctl: 运行失败:
  身份被占用：该身份已有进程在运行（同数据目录不支持多程序并行），如需切换请先停止
  另一进程；锁=<data-dir>/identity.lock…`。持锁者是 chat serve，需求方是 chat send；
  排障＝停掉同数据目录另一 chat 进程，锁随进程正常退出（SIGINT/SIGTERM）释放。
  node 守护与 chat 域进程身份根不同，可共存。

### 1.4 前置条件矩阵

| 前置 | 适用命令 | 不满足时的表现 |
|---|---|---|
| cargo 在 PATH（`$HOME/.cargo/bin`） | 二进制构建（cargo build/clippy/test） | `cargo: command not found`（退出 127）；先 `export PATH=$HOME/.cargo/bin:$PATH` |
| macOS 屏幕录制授权 | gui screenshot/record、scripts/ops/ui-regression.sh | 退出 1：CAPTURE_PERMISSION_DENIED（HTTP 403），PNG/GIF 不产出；GUI 重编译后 TCC 授权记录可能失效需重新授权（系统设置 > 隐私与安全性 > 屏幕录制），OS 级授权须人完成 |
| 无（离线可跑） | config、profile、chat friends/history/media、chat serve、identity init/show/reset、log tail/path/clear、metrics get、update check/open、node status、acp allow/deny/list、llm-share allow/deny/allowlist、llm-share ledger list、llm-share receipt verify、llm-share offer show | —— |
| 本机身份已初始化（<data-dir>/p2p-data/key.seed） | llm-share offer publish、llm-share ledger balance | 退出 1：节点身份加载失败；offer publish 不代生成身份；正向门面是 p2pctl identity init（幂等，显式创建后即可重试） |
| 对端在线可达 | chat send（真正送达）、peer dial/connect/ping | chat send 退出 1：超时未送达 status=Pending / 对端身份不符快速失败 status=Failed（均保留本机记录，见 chat send 条目与附录A）；peer 域退出 1 |
| 节点守护进程运行 | peer connect/disconnect/ping/dial、metrics get 实时值 | 退出 1：连接节点守护进程失败；metrics get 例外：返回全零不报错 |
| GUI 进程运行 | gui 全域（status/screenshot/record/navigate/invoke/page/action） | 退出 1（控制通道不可达） |
| GUI 日志目录存在（默认自动） | log tail/clear | 退出 1，文件不存在类错误 |

## 2. AI 意图 → 命令映射表

| 意图 | 命令 |
|---|---|
| 发消息 | `p2pctl chat send --peer <PEER_ID> --text "..." --json` |
| 查好友 / 加好友 / 删好友 | `chat friends list` / `chat friends add <PEER_ID>` / `chat friends remove <PEER_ID>` |
| 看消息历史 | `chat history --peer <PEER_ID> --json` |
| 两节点聊天 E2E 最小拓扑 / chat 与守护双身份说明 | 见附录A（B 起 chat serve → A 用其 chat peerId+监听地址加好友 → send 断言 delivered） |
| 查附件落盘路径 | `chat media file --peer <PEER_ID> --message-id <ID> --json` |
| 看节点状态 | `p2pctl node status --json` |
| 启动 / 停止节点 | `node start` / `node stop` |
| 测连通 / 拨号 / 挂断 | `peer ping <PEER_ID>` / `peer dial "<PEER_ID>@<ADDR>"` / `peer disconnect <PEER_ID>` |
| 查地址簿与在线态 | `peer list --json` |
| 查发现缓存 / 邻居（谁在网、地址从哪来） | `discovery list --json` |
| 查中继会话与水位 | `relay status --json` |
| 看节点守护进程日志 | `node log tail --lines 100 --json` |
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
| 给 peer 授予 agent 访问 | `acp allow <PEER_ID> --scope sandbox --json`（写，须人确认） |
| 撤销 peer 授权 | `acp deny <PEER_ID>`（写，须人确认；不存在条目报错退出 1） |
| 查看授权策略表 | `acp list --json` |
| 把借方加入出借 allowlist（可带模型白名单） | `llm-share allow <PEER_ID> --model <M> --json`（写，须人确认；缺 --model 不限模型） |
| 把借方移出 allowlist / 查 allowlist | `llm-share deny <PEER_ID>`（写，须人确认）/ `llm-share allowlist --json` |
| 签名发布能力声明 / 查看生效声明与剩余 TTL | `llm-share offer publish --model <M> --spare <M>=<N> --period-ends <DATE> --json`（写，须人确认）/ `llm-share offer show --json` |
| 查本机流水 / 净差视图 | `llm-share ledger list --json` / `llm-share ledger balance --json`（按 lender+period 切分） |
| 离线验签收据 | `llm-share receipt verify <PATH> --pubkey <BASE58>`（FAIL 退出 1，stdout 有 verdict 与原因） |
| 重置身份（红线） | `identity reset`——不可逆，见 §3 |

## 3. 开场提示词模板（整段贴给 LLM 即可）

```text
你是 p2p 节点的运维助手，通过 p2pctl 命令行工具操作 p2p。请严格遵守：

【工具认知】
- 可执行文件：apps/cli/target/debug/p2pctl（先 export PATH=$HOME/.cargo/bin:$PATH 再 cargo build --manifest-path apps/cli/Cargo.toml 构建若不存在；cargo 不在 PATH 会报 command not found）。
- 命令面：node|chat|config|profile|peer|gui|identity|log|metrics|update|acp|llm-share 十二域，共 45 个叶子命令。
- 每个命令先跑 --help 确认参数，再执行；官方命令参考见 docs/ops/p2pctl-ai-guide.md。
- 输出：默认人读文本（key=value 行），加 --json 得结构化 JSON（camelCase）。
- 退出码：0 成功；1 运行失败（stderr 前缀 "p2pctl: 运行失败: "）；2 用法错误。失败时先读 stderr 再决定下一步，不要盲目重试。

【安全边界（最高优先级）】
1. 只读命令优先：node status / chat friends list / chat history / config get / metrics get /
   log tail / gui status / update check 等查询类命令可自由执行。
2. 写操作必须先征得人确认再执行：chat send / chat friends add|remove / config save /
   profile save / node start|stop / peer dial|connect|disconnect / log clear / acp allow|deny / gui navigate /
   llm-share allow|deny|offer publish。
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
- **ACP 授权面**：acp allow/deny 直写节点策略表（<data-dir>/acp-policy.json），
  执行前必须向人复述目标 PeerId 与 scope 并获确认；deny 对不存在条目报错退出 1，
  属预期行为（默认拒绝语义，非故障）。
- **LLM 共享面**：llm-share allow/deny 直写出借方 allowlist（<data-dir>/llm-share/allowlist.json，
  默认拒绝语义），offer publish 以本机身份种子签名并落盘声明信封；执行前必须向人复述
  目标 PeerId / 模型白名单 / 闲量与账期参数并获确认。签名密钥只在 p2p-identity 种子文件
  （0600）中，AI 不得读取、打印或迁移其内容。

## 5. 与 GUI 的关系

p2pctl 是 GUI（p2p-console，Tauri 应用）命令面的等价 CLI，由 `scripts/check/cli-parity.sh`
守卫对等；`gui` 域是控制 GUI 本身的原语（需 GUI 进程运行），其余域与 GUI 各页面读写
同一份数据。GUI 数据目录（macOS）`~/Library/Application Support/com.p2p.console`，前端
日志 `~/Library/Logs/com.p2p.console/frontend.log`，均可用 `--gui-data-dir`/`--log-dir` 覆盖。

## 6. 命令面全目录（49 命令）

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
--json（运行中，含 peerId/listenAddrs）：
```
{"running":true,"pid":81444,"peerId":"aogbzDcMk5VeRUVkjLK8kLHHv4FWbaeQkg57ErxKmcq","listenAddrs":["127.0.0.1/u52063","127.0.0.1/t59667"],"uptimeSecs":3612,"logPath":"<data-dir>/daemon.log","dataDir":"<data-dir>","degraded":false,"reason":""}
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

### p2pctl node log tail
用途：读节点守护进程日志 `<data-dir>/daemon.log` 末尾 N 行（观测只读）。前置：无——文件缺失输出空行集（首启前合法状态），不报错。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --lines | int | 否 | 200（上限 1000） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：逐行原样输出日志行（守护进程 stderr 落盘内容，含 `p2pctl-daemon: running pid=...` 启动行）。
--json：`{"path":"<data-dir>/daemon.log","lines":["..."]}` 形态。
注意：本命令读 daemon.log；GUI 前端日志 frontend.log 属 `log` 域，两路不混。

### p2pctl chat friends list
用途：列出全部好友（默认按分组展示，未分组置底）。前置：无（空簿输出"好友簿为空（或该分组无成员）"）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --group | string | 否 | 无（只显示该分组；空串 = 未分组；省略 = 全部按分组展示） |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
共 1 位好友
[测试组] - Bobby 11111111111111111111111111111111 addrs=[] note=hi group=测试组
```
--json（单行紧凑）：
```
[{"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj","nickname":"Bob","addrs":[],"note":null,"group":"测试组"}]
```

### p2pctl chat friends add
用途：发送好友邀请（邀请制：对方同意前不建好友；delivered 判送达/挂起；重复邀请幂等刷新）。前置：无；PEER_ID 不得是本机 chat 身份、不得已是好友。
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
已发送好友邀请 Alice（54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4）：已送达，等待对方同意
```
--json：
```
{"invite":{"peerId":"54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4","nickname":"Alice","addrs":[],"note":null,"direction":"out","tsMs":1788563000000,"delivered":true},"delivered":true}
```
退出码：PEER_ID 非 32 字节 base58 → 1（PeerId 非法）；PEER_ID 为本机 chat 身份 → 1（不能与自己通信）；已是好友 → 1（已是好友）。

### p2pctl chat friends invites list
用途：列出邀请（direction out=本机待对方同意 / in=对方待本机处理；delivered 标记送达状态）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
共 1 条邀请
- Alice 54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4（待本机处理，已送达）
```
--json：
```
[{"peerId":"54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4","nickname":"Alice","addrs":["127.0.0.1/u41001"],"note":null,"direction":"in","tsMs":1788563000000,"delivered":true}]
```
退出码：无邀请时输出空数组/提示，退出码 0。

### p2pctl chat friends invites accept
用途：同意来邀——本机立即建立好友（双向关系本侧），并回投 ACCEPT 让对方建簿。前置：存在该 PEER_ID 的待处理来邀（in）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --nickname | string | 否 | ""（空 = 回退 PeerId 缩略，如 `abcdEFGH...`，与 update --nickname 空串同口径） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已同意好友邀请：Alice（54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4）
```
--json：
```
{"ok":true}
```
退出码：无待处理来邀 → 1（无待处理邀请）；成功 → 0。

### p2pctl chat friends invites reject
用途：拒绝来邀并通知对方（通知尽力而为，对端离线时由重连收敛）。前置：存在待处理来邀。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已拒绝好友邀请: 54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4
```
退出码：无待处理来邀 → 1。

### p2pctl chat friends invites cancel
用途：撤回本机待同意邀请（out）；不在簿幂等返回 ok=false。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已撤回好友邀请: 54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4
```
--json：
```
{"ok":true}
```
退出码：无待同意邀请 → 0（幂等，ok=false）。
### p2pctl chat friends update
用途：更新好友的分组/昵称/备注/可拨地址补丁（至少提供一项）。前置：PEER_ID 在簿。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string | 是 | —— |
| --group | string | 否 | 无（trim 后 ≤32 字符；空串 = 移出分组） |
| --nickname | string | 否 | 无（trim 后 ≤64 字符；空串回退 PeerId 缩略） |
| --note | string | 否 | 无（空串 = 清空） |
| --addr | string | 否 | 无（可重复；ip/u端口 或 ip/t端口，整组替换，非法地址拒绝） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已更新好友 Bobby（11111111111111111111111111111111）group=测试组
```
--json：
```
{"peerId":"11111111111111111111111111111111","group":"测试组","nickname":"Bobby","note":"hi","addrs":["127.0.0.1/u41001"]}
```
退出码：至少提供一项补丁，全缺 → 1；PEER_ID 不在簿 → 1；--addr 非法 → 1（不落盘）。

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
用途：发送消息（--text 文本或 --file 附件，二选一）。前置：对端在线可达才真正送达；PEER_ID 不得是本机 chat 身份。离线投递语义（PR1）：对端不可达时消息排队（status=pending），对端恢复可达后由 serve 补投泵自动补投，也可 `chat outbox flush` 手动补投。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --peer | string | 是 | —— |
| --text | string | 与 --file 二选一 | —— |
| --file | path | 与 --text 二选一 | —— |
| --kind | text/image/audio/video/file | 否 | file（mime 按扩展名推断） |
| --mime | string | 否 | 按扩展名推断 |
| --name | string | 否 | 取文件名 |
| --reply-to | string | 否 | 无 |
| --timeout-secs | int | 否 | 30（送达后排空积压预算，上限 30） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（对端离线，排队，秒级返回）：
```
已排队 peer=54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4 id=425a11cd-4b71-4397-ba6e-4771e1414a0d status=Pending（对端离线，恢复可达后自动补投）
```
--json：
```
{"message":{"status":"pending","id":"425a11cd-4b71-4397-ba6e-4771e1414a0d"},"delivered":false,"flushedOutbox":0}
```
退出码：delivered=true → 0；pending（对端离线已排队）→ 0 且 delivered=false（自动化以 delivered/status 判定投递结果）；failed → 1（快速真失败，典型成因是对端 peerId 填了守护身份而非 chat 身份、ACK 校验失败等；本机记录保留，可经 `chat outbox flush` 补救）；PEER_ID 为本机身份 → 1。

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

### p2pctl chat outbox list
用途：按对端列出 1:1 行箱——积压（pending/failed 摘要）与本端发出且已确认（delivered）计数。前置：无（无积压且无已投记录的对端不列）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
共 1 个对端
- peer=54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4 pending=1 failed=0 delivered=2 entries=[Pending[425a11cd-4b71-4397-ba6e-4771e1414a0d]]
```
--json：
```
[{"peerId":"54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4","pending":1,"failed":0,"delivered":2,"entries":[{"id":"425a11cd-4b71-4397-ba6e-4771e1414a0d","kind":"text","tsMs":1788563000000,"status":"pending"}]}]
```
退出码：空行箱输出 `[]`/提示，退出码 0。

### p2pctl chat outbox flush
用途：手动补投行箱积压（缺省全部含积压对端；--peer 指定单个），逐对端回报补投/剩余条数。前置：无。对端不可达时条目保持 pending（正常闭环，不算命令失败）；serve 启动与周期补投泵、对端连入窗口亦会自动补投。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --peer | string | 否 | 无（缺省全部含积压对端） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
peer=54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4 补投 1 条 剩余 0 条
合计补投 1 条，剩余 0 条（对端不可达时保持 pending 待自动补投）
```
--json：
```
{"peers":[{"peerId":"54wweGDKHFKfCwsEYR37yCJeJQQ5ykD9KvcPGJLmiDy4","flushed":1,"remaining":0,"error":null}],"flushedTotal":1,"remainingTotal":0}
```
退出码：无积压 → 0（提示无需补投）；补投后仍有 remaining（对端不可达）→ 0（结果如实回报）；store IO 异常 → 0 但 error 字段/文本留观测；--peer 非法 → 1。

### p2pctl chat serve
用途：常驻运行聊天节点，输出 peerId 与监听地址后等待信号（E2E/守护支撑）。前置：无；运行期间持有 <data-dir>/identity.lock，与同数据目录 chat send 互斥（见 §1.3）。输出的 peerId 是 chat 身份，与 node start 守护 peerId 不同根，聊天收发一律用本条输出的 peerId 与 listen 地址（配方见附录A）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --data-dir | path | 否 | ./p2p-data |
| --quic-port | int | 否 | 端口记忆：首启随机并落盘 serve.json，重启沿用上次端口；显式指定优先并覆盖记忆（绑定失败回退随机并留 stderr 观测） |
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
注意：常驻命令，AI 应以人确认后使用，且用完发 SIGINT/SIGTERM 退出。启动时对含积压对端做首趟补投（约 2s 延迟），此后每 30s 周期补投（PR1/F3）。

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
文本（成功）：`pong from <PEER_ID> rtt_ms=<数值> rtt_us=<数值>` 形态 key=value 行；
亚毫秒 RTT 以 0.1ms 精度呈现（如 `rtt_ms=0.4 rtt_us=412`），不再恒 0。
stderr（守护未运行）：
```
p2pctl: 运行失败: 连接节点守护进程失败: No such file or directory (os error 2)
```

### p2pctl peer list
用途：列出地址簿与在线态（守护进程观测注册表只读查询）。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：首行 `total=<N> connected=<M>` 汇总；逐对端一行
`peer=<PEER_ID> connected=<bool> source=<mdns|rendezvous|manual> lastSeenMs=<ts> firstSeenMs=<ts>`，
地址行 `addr=<ADDR>` 紧随其后。
--json：`{"peers":[{"peerId","addrs","source","connected","lastSeenMs","firstSeenMs"}],"total","connected"}` 形态。
注意：只反映本守护进程启动后的对端痕迹（事件聚合，语义同 GUI 邻居表）。

### p2pctl discovery list
用途：发现缓存只读查询（邻居与登记地址，口径同 GUI 发现页）。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：首行来源计数 `total=<N> mdns=<A> rendezvous=<B> manual=<C> connected=<D>`，
逐邻居 `neighbor=<PEER_ID> source=... connected=... lastSeenMs=... firstSeenMs=...` + `addr=` 行。
--json：`{"neighbors":[...],"stats":{"total","connected","mdns","rendezvous","manual"}}` 形态。

### p2pctl relay status
用途：中继会话/水位只读快照。前置：节点守护进程运行。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
relaySessionsActive=2 relayReconnects=0 activeConnections=3
dialPunch ok=1 fail=0
dialRelay ok=0 fail=2
relayAddr=43.240.223.138/u3403
```
--json：`{"relaySessionsActive","relayReconnects","activeConnections","dialPunchOk","dialPunchFail","dialRelayOk","dialRelayFail","relayAddrs"}` 形态。

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
用途：截图主窗口内容并落盘 PNG。前置：GUI 进程运行；输出路径须绝对；macOS 屏幕录制授权（缺失时退出 1：CAPTURE_PERMISSION_DENIED，HTTP 403；GUI 重编译后 TCC 授权记录可能失效，需人在 系统设置 > 隐私与安全性 > 屏幕录制 重新授权，见 §1.4）。
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
用途：开始录屏（产物 GIF）。前置：GUI 进程运行；输出路径须绝对；屏幕录制授权同 gui screenshot（CAPTURE_PERMISSION_DENIED 见 §1.4）。
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
用途：查询当前页语义 descriptor：name/description 与 actions 表格（动作与页面按钮同源走 store/IPC，非 DOM 模拟）。前置：GUI 进程运行；当前页未进注册表时服务端 PAGE_NOT_REGISTERED 拒绝（2026-09-04 实测 dashboard 已注册，正常返回含 start/stop 动作的 descriptor，勿再以 dashboard 当未注册反例）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --gui-data-dir | path | 否 | GUI 应用数据目录 |
文本（头两行为协议定位，每动作一行，args 标注「类型,必填性」，危险动作带 [confirm] 标记）：
```
page=chat
schemaVersion=1
name=chat
description=IM 聊天页：好友会话文本发送与好友管理
actions=3
- sendText: 向好友发送文本（乐观更新，与聊天输入框同源）
  args: peer(string,必填) text(string,必填)
- removeFriend: 移除好友（不删本地消息历史），与移除确认框同源 [confirm]
  args: peer(string,必填) confirm(boolean,必填)
```
--json：服务端全量 {schemaVersion,page,descriptor}（descriptor 含 args schema 与 state 快照）。
退出码：当前页未注册 → 1（PAGE_NOT_REGISTERED，错误信息含可用页清单）；前端未回执 → 1（PAGE_TIMEOUT）。

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
文本（回包 result 的人读 JSON，CLI 自动生成 requestId，chat addFriend 例）：
```
{
  "peerId": "11111111111111111111111111111111111111111111",
  "nickname": "gc4-e2e",
  "addrs": [],
  "note": null
}
```
--json：同源结构化 {requestId,result}（result 即动作返回值原样）。
退出码：非当前页 → 1（结构化错误含「gui navigate <页面>」指引）；危险动作缺 confirm=true → 1（ACTION_CONFIRM_REQUIRED 透传）；动作不存在 → 1（ACTION_NOT_FOUND，含可用动作清单）；页面名非法 → 1（PAGE_NOT_FOUND，含可用页清单）；前端未回执 → 1（PAGE_TIMEOUT）。

### p2pctl identity init
用途：显式创建本机节点身份（node 域，<data-dir>/p2p-data/key.seed，文件 0600、目录 0700）。幂等：身份已存在时输出既有身份并退出 0，不重建。前置：无（新目录可作首命令，替代借道 node start/chat serve 造身份的旧路径）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（首建）：
```
身份已创建 peer=<peerId>
seed=<data-dir>/p2p-data/key.seed
pubkey=<公钥 base58>
mode=0600
```
文本（幂等重入）：首行变为「身份已存在 peer=<peerId>」，其余同上。
--json：
```
{"created":true,"peerId":"…","publicKey":"…","seedPath":"…","mode":"0600"}
```
退出码：成功（含幂等重入）→ 0；种子创建/加载失败 → 1。

### p2pctl identity show
用途：只读查看本机身份（node|chat 双身份），输出 peerId 与公钥。不起进程、不占 identity.lock，节点/聊天运行中可安全调用。前置：目标域身份已存在（缺失退出 1，不代生成）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --domain | enum(node\|chat) | 否 | node |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
domain=node
peer=<peerId>
pubkey=<公钥 base58>
seed=<种子路径>
```
双根：node=<data-dir>/p2p-data/key.seed（守护身份）；chat=<data-dir>/key.seed（聊天身份，同 chat serve 首行输出的 peerId）。
--json：
```
{"domain":"node","peerId":"…","publicKey":"…","seedPath":"…"}
```
退出码：成功 → 0；目标域身份不存在 → 1；种子加载失败 → 1。

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
| --data-dir | path | 否 | 无（同 --log-dir 的别名；同时给出时优先于 --log-dir） |
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
| --data-dir | path | 否 | 无（同 --log-dir 的别名；同时给出时优先于 --log-dir） |
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
| --data-dir | path | 否 | 无（同 --log-dir 的别名；同时给出时优先于 --log-dir） |
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

### p2pctl acp allow
用途：授予 peer 访问本机 agent 的授权（upsert：条目已存在则为更新并刷新 grantedAt）。前置：无（纯本地策略表，离线可跑）；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string（base58，32 字节） | 是 | —— |
| --scope | sandbox 或 workspace | 否 | sandbox |
| --allow-mcp | string（可重复；mcpServers 白名单按名引用） | 否 | 无 |
| --ask-route | remote_gui 或 owner_local | 否 | remote_gui |
| --note | string | 否 | 无 |
| --fingerprint | string（TOFU 指纹显式登记进策略表） | 否 | 空 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已授权 peer=HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj（新建条目）
scope=sandbox
allow_mcp=fs,web
ask_route=remote_gui
granted_at=2026-09-04T06:00:00Z
```
--json：
```
{"created":true,"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj","scope":"sandbox","allowMcp":["fs","web"],"askRoute":"remote_gui","grantedAt":"2026-09-04T06:00:00Z"}
```
退出码：PeerId 非法（非 base58 或解码后非 32 字节）→ 1；策略表损坏 → 1（可读报错，禁止静默回退空表）。

### p2pctl acp deny
用途：撤销 peer 授权（删除策略表条目，回到默认拒绝语义）。前置：无（离线可跑）；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string（base58，32 字节） | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已撤销授权 peer=HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj（回到默认拒绝）
```
--json：
```
{"removed":true,"peerId":"HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj"}
```
退出码：条目不存在 → 1（明确报错不静默）；PeerId 非法 → 1；策略表损坏 → 1。

### p2pctl acp list
用途：表格列出全部授权条目（peer/scope/allowMcp/askRoute/grantedAt/指纹/note）。前置：无（离线可跑；策略文件缺失视为空表）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（有条目）：
```
共 1 条授权
PEER                                          SCOPE    ALLOW_MCP  ASK_ROUTE   GRANTED_AT            FINGERPRINT  NOTE
HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj  sandbox  fs,web     remote_gui  2026-09-04T14:49:29Z  ff00         nb
```
文本（空表）：
```
策略表为空（默认拒绝：未列入条目的 peer 一律无授权）
```
--json（空表）：
```
{"peers":[]}
```
退出码：策略表损坏 → 1（可读报错）。

### p2pctl llm-share allow
用途：把借方加入出借方 allowlist（upsert：条目已存在则为更新并刷新 grantedAt）。前置：无（离线可跑，纯本地 allowlist）；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string（base58，32 字节） | 是 | —— |
| --model | string（可重复；模型白名单） | 否 | 无（= 不限模型） |
| --note | string | 否 | 空 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已加入 allowlist peer=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd（新建条目）
models=gpt-4o,deepseek-v3
note=首批白名单
granted_at=2026-09-04T19:15:09Z
```
--json：
```
{"created":true,"peerId":"52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd","models":["gpt-4o","deepseek-v3"],"note":"首批白名单","grantedAt":"2026-09-04T19:15:09Z"}
```
语义：默认拒绝——allowlist 无条目的借方一律不可用（G4 准入）。文件 <data-dir>/llm-share/allowlist.json，原子落盘。
退出码：PeerId 非法（非 base58 或解码后非 32 字节）→ 1；模型名为空 → 1；文件损坏 → 1（可读报错，禁止静默回退空表）。

### p2pctl llm-share allowlist
用途：列出出借方 allowlist 全部条目（BTreeMap 序，输出稳定）。前置：无（离线可跑；文件缺失视为空表）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（有条目）：
```
共 1 条 allowlist 条目
52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd  models=gpt-4o  note=  granted_at=2026-09-04T19:15:09Z
```
文本（空表）：
```
allowlist 为空（默认拒绝：未列入条目的借方一律不可用）
```
--json（空表）：
```
{"peers":[]}
```
退出码：文件损坏 → 1（可读报错）。

### p2pctl llm-share deny
用途：把借方移出 allowlist（删除条目，回到默认拒绝）。前置：无（离线可跑）；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PEER_ID> | 位置参数 string（base58，32 字节） | 是 | —— |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
已移出 allowlist peer=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd（回到默认拒绝）
```
--json：
```
{"removed":true,"peerId":"52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd"}
```
退出码：条目不存在 → 1（明确报错不静默，属默认拒绝语义非故障）；PeerId 非法 → 1；文件损坏 → 1。

### p2pctl llm-share offer publish
用途：组装能力声明（§5.2 全字段：模型清单/闲量/账期末/TTL/速率限额/retention）并以本机身份 Ed25519 签名发布。前置：本机身份已初始化（节点身份数据目录 key.seed，缺失退出 1 不代生成）；写操作须人确认。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --model | string（可重复，至少一个） | 是 | 无 |
| --spare | kv（model=N，token 数，可重复且须覆盖全部 --model 且 N>0） | 是 | 无 |
| --period-ends | string（YYYY-MM-DD 账期截止日） | 是 | —— |
| --max-per-req | kv（model=N，单请求 max_tokens 上限，可重复） | 否 | 无（= 未显式设限） |
| --rpm | int（每分钟请求上限） | 否 | 10 |
| --concurrency | int（并发上限） | 否 | 2 |
| --ttl | int（声明有效期秒数，自签发起算） | 否 | 3600 |
| --retention | string（数据留存自述，§7.3 如实告知） | 否 | none |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
--json：
```
{"peer":"7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk","models":["gpt-4o","deepseek-v3"],"spare":{"deepseek-v3":999999999,"gpt-4o":1500000},"periodEnds":"2026-09-30","maxPerReq":{"gpt-4o":128000},"rateLimit":{"rpm":10,"concurrency":2},"ttl":3600,"retention":"none","issuedAt":1788549309,"expiresAt":1788552909,"file":"/tmp/demo/p2p-data/llm-share/offer.json"}
```
文本：同字段 key=value 行（spare/max_per_req 为 model=N 逗号串，rate_limit=rpm=N,concurrency=N）。
落盘：签名信封（含 pubkey/sig，公开数据）写 <data-dir>/llm-share/offer.json，tmp+rename 原子写。
退出码：身份缺失 → 1；--model 为空/--spare 未覆盖某模型或为 0/引用未声明模型/重复声明 → 1；日期格式非法 → 1；rpm/concurrency/ttl 非正 → 1。

### p2pctl llm-share offer show
用途：查看当前生效声明与剩余 TTL（status=live/expired/not_yet_valid/peer_mismatch/bad_signature）。前置：无（离线可跑）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
peer=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk
models=gpt-4o,deepseek-v3
spare=deepseek-v3=999999999,gpt-4o=1500000
period_ends=2026-09-30
max_per_req=gpt-4o=128000
rate_limit=rpm=10,concurrency=2
ttl=3600s
retention=none
issued_at=1788549309
expires_at=1788552909
status=live
remaining_secs=3600
file=/tmp/demo/p2p-data/llm-share/offer.json
```
--json：同字段 camelCase（声明本体 + remaining_secs/status/file）。
退出码：从未发布 → 1（提示先 publish）；信封文件损坏 → 1；TTL 过期不是错误：status=expired、remaining_secs≤0 照常输出。

### p2pctl llm-share ledger list
用途：查询本机双边流水明细（§5.1 收据，append-only 存储序）。前置：无（离线可跑；流水文件缺失视为空账）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --lender | string（PeerId 过滤） | 否 | 不过滤 |
| --borrower | string（PeerId 过滤） | 否 | 不过滤 |
| --period | string（账期过滤，如 2026-09） | 否 | 不过滤 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
共 2 条流水
req_id=0198c0de-0000-7000-8000-000000000001  period=2026-09  lender=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd  borrower=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk  model=gpt-4o  tokens=6912 (in=1234 out=5678)  estimated=false  ts=1788480000
req_id=0198c0de-0000-7000-8000-000000000002  period=2026-09  lender=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk  borrower=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd  model=deepseek-v3  tokens=40 (in=20 out=20)  estimated=false  ts=1788480000
```
--json：
```
{"count":2,"entries":[{"reqId":"0198c0de-0000-7000-8000-000000000001","period":"2026-09","lender":"52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd","borrower":"7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk","model":"gpt-4o","input":1234,"output":5678,"tokens":6912,"estimated":false,"ts":1788480000}]}
```
退出码：流水文件损坏 → 1（可读报错）。

### p2pctl llm-share ledger balance
用途：本机净差视图（§3.2）：本机参与的条目按 (lender, period) 聚合，本机为 lender 记正（lent_out）、为 borrower 记负（borrowed），net = lent_out − borrowed，对齐账本 net 口径。前置：本机身份已初始化（净差视角取本机 PeerId）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --period | string（只统计该账期） | 否 | 全部账期分行 |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
peer=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk 净差视图（lender 正 / borrower 负）
lender=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd  period=2026-09  lent_out=0  borrowed=6912  net=-6912  entries=1
lender=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk  period=2026-09  lent_out=40  borrowed=0  net=40  entries=1
```
--json：
```
{"peer":"7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk","rows":[{"lender":"52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd","period":"2026-09","lentOut":0,"borrowed":6912,"net":-6912,"entries":1}]}
```
退出码：身份缺失 → 1；流水文件损坏 → 1；本机未参与任何流水 → 正常输出空 rows。

### p2pctl llm-share receipt verify
用途：指定收据文件离线 Ed25519 验签（§5.1，MVP A3）：先校验公钥与 lender PeerId 绑定（PeerId = sha256(pubkey)），再验规范化 payload 签名；任何字段篡改必 FAIL。--pubkey 缺省读本机节点身份公钥（与 offer publish 签名同根）；显式给出时取其 offer 信封 pubkey 字段（base58）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| <PATH> | 位置参数 path（收据文件，§5.1 wire JSON） | 是 | —— |
| --pubkey | string（出借方公钥 base58，32 字节） | 否 | 本机节点身份公钥（<data-dir>/p2p-data/key.seed） |
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本（PASS）：
```
verdict=PASS
reason=验签通过
req_id=0198c0de-0000-7000-8000-000000000001
period=2026-09
lender=52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd
borrower=7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk
model=gpt-4o
usage=input=1234,output=5678
estimated=false
ts=1788480000
```
文本（FAIL，stdout 照常输出完整报告，进程退出 1）：
```
verdict=FAIL
reason=验签失败: receipt signature invalid: req_id=0198c0de-0000-7000-8000-000000000001
（req_id/period/lender/borrower/model/usage/estimated/ts 各行同 PASS 形态）
```
--json：同字段 camelCase（verdict/reason/reqId/period/lender/borrower/model/input/output/estimated/ts）。
退出码：verdict=PASS → 0；verdict=FAIL（签名无效/公钥不绑定）→ 1（报告已先输出，stderr 再给一行失败信号）；收据文件不存在/损坏 → 1；公钥非法（非 base58 或解码后非 32 字节）→ 1；缺省 --pubkey 且本机身份未初始化 → 1（含 identity init 指引）。
### p2pctl group create
用途：建群。校验成员 ⊆ 好友簿、≤32、不含本机；群名 trim 后 1..=64 字符。建群后对每个初始成员推 roster（成员离线经 goutbox 补投，命令不失败）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --name | string | 是 | —— |
| --member | string（可重复） | 否 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：成员不在好友簿 / 群超员 / 群名非法 → 1。

### p2pctl group list
用途：列出全部群（含 left/kicked/disbanded，GUI 按 state 过滤置底）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |

### p2pctl group invite
用途：邀请成员（owner-only）。受邀者须 ⊆ 好友簿且不在群；群 <32；rev+1 推全体（含新成员）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --member | string（可重复） | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：非 owner / 受邀者不在好友簿或已在群 / 超员 → 1。

### p2pctl group kick
用途：移除成员（owner-only）。rev+1 推余员，并向被移者发 G_KICK(reason=kicked)。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --member | string | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：非 owner / 成员不在群 / 不能移除群主 → 1。

### p2pctl group leave
用途：退群。本端 state=left（历史保留），并向 owner 发 G_LEAVE（owner 离线经 goutbox 补投）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：owner 调用 → 1（群主不能退群，请改用 disband）；群已非 active → 1。

### p2pctl group rename
用途：改名（owner-only）。群名 trim 后 1..=64 字符；rev+1 推 roster。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --name | string | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：非 owner / 群名非法 → 1。

### p2pctl group disband
用途：解散（owner-only）。rev+1，对全体在册成员发 G_KICK(reason=disbanded)；本端 state=disbanded。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：非 owner / 群已非 active（含重复解散）→ 1。

### p2pctl group send
用途：发群消息（--text 文本或 --file 附件，二选一）。fan-out 至全体其他成员；未全员送达按退出码 1 失败（报告仍输出，成员离线经 goutbox 补投）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --text | string | 与 --file 二选一 | —— |
| --file | path | 与 --text 二选一 | —— |
| --kind | text/image/audio/video/file | 否 | file（mime 按扩展名推断） |
| --mime | string | 否 | 按扩展名推断 |
| --name | string | 否 | 取文件名 |
| --reply-to | string | 否 | 无 |
| --timeout-secs | int | 否 | 30 |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：未知群/群非 active/校验非法 → 1；超时未全员送达 → 1（acked/recipients 可判部分送达）。

### p2pctl group history
用途：读群历史（time desc；limit 缺省 50 上限 100；beforeId 严格更早游标）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --before-id | string | 否 | —— |
| --limit | int | 否 | 50 |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |

### p2pctl group media file
用途：查询群附件落盘绝对路径（media/<groupId>/，仅本端展示）。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --group | string | 是 | —— |
| --message | string | 是 | —— |
| --json | flag | 否 | off |  |  |  |
| --data-dir | path | 否 | ./p2p-data |  |  |  |
退出码：消息不存在或非附件类型 → 1。

<!-- AI-DOCS-SYNC:END -->

## 附录A. 两节点聊天 E2E 最小拓扑（chat serve 双身份模型）

来源：2026-09-04 AI 操作者试运行（docs/notes/ai-pilot-findings.md，摩擦 F2/F3/F4）。
本节是可照抄的配方，消除「靠报错猜拓扑」的摩擦。

### A.1 双身份模型（先读再动手）

- 每个数据目录有两套身份：**守护身份**（`node start` 输出的 peerId，根在
  `<data-dir>/p2p-data`）与 **chat 身份**（`chat serve` 输出的 peerId，根在
  `<data-dir>/chat`）。两者不同根不同值，属正常现象。
- 聊天收发（friends add / send、history --peer）全程使用 **chat 身份** peerId 与
  **chat serve 的监听地址**；把守护 peerId / 守护地址当聊天对端，`chat send` 会
  立即 status=Failed 快速失败。
- `chat send` 是一次性命令，不需要本机守护进程，但要求同数据目录没有其他 chat
  进程持 `identity.lock`（chat serve 与 chat send 互斥，见 §1.3）。

### A.2 配方（A 发给 B，命令与输出形态均为 2026-09-04 实测）

```bash
# 0. 两端各用独立数据目录（示例 /tmp/ai-pilot-a、/tmp/ai-pilot-b）
export PATH=$HOME/.cargo/bin:$PATH && cargo build --manifest-path apps/cli/Cargo.toml

# 1. B 端起常驻 chat 节点，记录首行 peer=<B 的 chat peerId> 与 listen=<两条监听地址>
p2pctl chat serve --data-dir /tmp/ai-pilot-b
#   chat 节点就绪 peer=<B-CHAT-PEER-ID> listen=127.0.0.1/u57844 127.0.0.1/t59850

# 2.（A 要收回信才需要）A 端临时起 serve 读本机 chat 身份，读完 Ctrl-C 停掉
#   （identity.lock 与后续 send 互斥，必须先停，见 A.3）
p2pctl chat serve --data-dir /tmp/ai-pilot-a   # 记下 peer=<A-CHAT-PEER-ID> 后 SIGINT

# 3. A 端加好友：peerId 用 B 的 chat 身份；addr 原样填第 1 步 listen 的两条地址
p2pctl chat friends add <B-CHAT-PEER-ID> \
  --addr 127.0.0.1/u57844 --addr 127.0.0.1/t59850 --data-dir /tmp/ai-pilot-a

# 4. A 端发送并断言送达：--json 输出 "delivered":true 即成功，记下返回的消息 id
p2pctl chat send --peer <B-CHAT-PEER-ID> --text "hello-from-A" --json \
  --data-dir /tmp/ai-pilot-a

# 5. B 端读回断言：同一消息 id、sender=them、文本与发送一致
p2pctl chat history --peer <A-CHAT-PEER-ID> --json --data-dir /tmp/ai-pilot-b

# 6. 收尾：serve 以 SIGINT/SIGTERM 停止（identity.lock 随进程退出释放）
```

### A.3 失败形态速查

| 现象 | 原因 | 处置 |
|---|---|---|
| `chat send` 报「身份被占用…锁=<data-dir>/identity.lock」退出 1 | 同数据目录已有 chat serve（或另一 chat 进程）持锁 | 停掉同数据目录另一 chat 进程再 send |
| `chat send` 秒级失败 status=Failed（delivered=false） | 对端 peerId 填了守护身份、对端未起 chat serve、addr 失效 | 按 A.1/A.2：peerId 取 chat serve 输出，addr 取 listen 行 |
| `chat send` 等满超时后 status=Pending | 对端进程不在/网络不可达 | 确认对端 serve 存活与地址后重发 |

## 附录B. 产品缺口建议（仅登记，不实现）

来自同次试运行（F5/F6）。本文档只登记建议，实现落地前不得写进 §6 命令目录，
命令名与行为以实现为准：

1. **chat 身份只读查询命令**：建议新增 `chat identity show`（或让 node status 等
   现有读命令顺带暴露本机 chat peerId）。动机：当前查本机 chat 身份只能临时起
   `chat serve` 读首行，而它持 identity.lock 与 chat send 互斥——「学身份必须先起
   占锁进程、学完还得停」自相矛盾；期望离线可读、不占锁。
2. **权限自检指引**：gui screenshot/record 与 scripts/ops/ui-regression.sh 挂在
   macOS 屏幕录制授权上，OS 级授权 AI 无法自助完成，且 GUI 重编译后 TCC 授权记录
   可能失效需重新授权。建议提供授权状态预检手段（自检命令或文档化探针步骤：先跑
   轻量 capture 探针再进截图/回归主流程），并把授权路径（系统设置 > 隐私与安全性
   > 屏幕录制）作为 CAPTURE_PERMISSION_DENIED 的标准前置指引。