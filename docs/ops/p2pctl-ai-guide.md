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
| 无（离线可跑） | config、profile、chat friends/history/media、chat serve、identity init/show/reset、log tail/path/clear、metrics get、update check/open、node status、acp allow/deny/list、acp share list、llm-share allow/deny/allowlist、llm-share ledger list、llm-share receipt verify、llm-share offer show、llm-share provider list/save/remove、llm-share share create/list/revoke | —— |
| 本机身份已初始化（<data-dir>/p2p-data/key.seed） | llm-share offer publish、llm-share ledger balance、llm-share borrow | 退出 1：节点身份加载失败；offer publish 不代生成身份；正向门面是 p2pctl identity init（幂等，显式创建后即可重试） |
| agent 节点身份已存在（<acp-data-dir>/identity/key.seed，由 acp-agent 首启生成） | acp share create | 退出 1：分享链接需要 agent 身份；CLI 不代生成，先启动一次 acp-agent |
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
| 拉好友入群（同意制）/ 处理入群邀请 | `group invites send --group <GID> --peer <PEER_ID> --json` / `group invites list --json` / `group invites accept <INVITE_ID>` / `group invites reject <INVITE_ID> --reason <R>`（写，须人确认） |
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
| 创建 agent 分享链接（限时/限激活次数） | `acp share create --scope sandbox --ttl-secs 86400`（写，须人确认；输出 JSON 含 share_id/token/link） |
| 查看分享台账 | `acp share list --json`（脱敏：无 token 原文/哈希） |
| 撤销分享 | `acp share revoke <SHARE_ID>`（写，须人确认；已绑定 peer 时级联删除其策略条目） |
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
- 命令面：node|chat|config|profile|peer|gui|identity|log|metrics|update|acp|llm-share 十二域，共 82 个叶子命令（以 ai-docs-sync 每次实测汇总行为准）。
- 每个命令先跑 --help 确认参数，再执行；官方命令参考见 docs/ops/p2pctl-ai-guide.md。
- 输出：默认人读文本（key=value 行），加 --json 得结构化 JSON（camelCase）。
- 退出码：0 成功；1 运行失败（stderr 前缀 "p2pctl: 运行失败: "）；2 用法错误。失败时先读 stderr 再决定下一步，不要盲目重试。

【安全边界（最高优先级）】
1. 只读命令优先：node status / chat friends list / chat history / config get / metrics get /
   log tail / gui status / update check 等查询类命令可自由执行。
2. 写操作必须先征得人确认再执行：chat send / chat friends add|remove / config save /
   profile save / node start|stop / peer dial|connect|disconnect / log clear / acp allow|deny /
   acp share create|revoke / gui navigate /
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
- **ACP 分享面**：acp share create/revoke 直写分享台账（<data-dir>/acp-shares.json）。
  create 输出的 token 原文只在本次 stdout 与链接里出现，AI 不得复述进日志或转存文件；
  scope=workspace 在 CLI 侧一律拒绝（无法确认 agent 已配 workspace-dir，fail-closed），
  需经 agent admin HTTP/GUI 创建。revoke 对不存在 share_id 报错退出 1，属预期行为。
- **LLM 共享面**：llm-share allow/deny 直写出借方 allowlist（<data-dir>/llm-share/allowlist.json，
  默认拒绝语义），offer publish 以本机身份种子签名并落盘声明信封；执行前必须向人复述
  目标 PeerId / 模型白名单 / 闲量与账期参数并获确认。签名密钥只在 p2p-identity 种子文件
  （0600）中，AI 不得读取、打印或迁移其内容。
- **公网外联告知（F8）**：默认配置下节点启动即连接公共设施——bootstrap（rendezvous
  跨网发现）、relay（中继兜底）、observation（公网地址观测，出厂 43.240.223.138 /
  121.196.193.177）；node start 的人读与 --json 输出逐类列出将连接的端点与意图。
  PeerId、监听/观测地址等元数据会经公共节点转发，对隐私敏感的部署必须向人如实说明。
  lan-only 出口：config save 置 lanOnly=true 后重启节点，不连任何公共设施（不拨
  bootstrap、不连 relay、不上报观测，仅 mDNS 局域网发现与直连）；start 声明转为
  「仅局域网(lan-only)」，node status 输出 lanOnly=true 供机械验证。

## 5. 与 GUI 的关系

p2pctl 是 GUI（p2p-console，Tauri 应用）命令面的等价 CLI，由 `scripts/check/cli-parity.sh`
守卫对等；`gui` 域是控制 GUI 本身的原语（需 GUI 进程运行），其余域与 GUI 各页面读写
同一份数据。GUI 数据目录（macOS）`~/Library/Application Support/com.p2p.console`，前端
日志 `~/Library/Logs/com.p2p.console/frontend.log`，均可用 `--gui-data-dir`/`--log-dir` 覆盖。

## 6. 命令面全目录（72 命令）

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
--json（运行中，含 peerId/listenAddrs/lanOnly）：
```
{"running":true,"pid":81444,"peerId":"aogbzDcMk5VeRUVkjLK8kLHHv4FWbaeQkg57ErxKmcq","listenAddrs":["127.0.0.1/u52063","127.0.0.1/t59667"],"uptimeSecs":3612,"lanOnly":false,"logPath":"<data-dir>/daemon.log","dataDir":"<data-dir>","degraded":false,"reason":""}
```

### p2pctl node start
用途：启动节点守护进程（读 gui-config.json，缺省用默认配置）。前置：无。
| 参数 | 类型 | 必填 | 默认 |
|---|---|---|---|
| --json | flag | 否 | off |
| --data-dir | path | 否 | ./p2p-data |
文本：
```
## A2A 域（agent 管理 + 授权管理）

### a2a list

列出全部 agent（本机发布的 + 远程发现的）。

```bash
p2pctl a2a list [--json] [--data-dir <path>]
```

- `--json`：输出 JSON 格式
- `--data-dir`：数据目录（默认 ./p2p-data）

### a2a publish

发布 agent（创建或更新可见性）。

```bash
p2pctl a2a publish --agent-id <id> --name <name> --description <desc> [--visibility public|private] [--json] [--data-dir <path>]
```

- `--agent-id`：agent ID（kebab-case，仅 [a-z0-9-]，<=32 字符）
- `--name`：agent 名称
- `--description`：agent 描述
- `--visibility`：可见性（默认 private）
- `--json`：输出 JSON 格式
- `--data-dir`：数据目录（默认 ./p2p-data）

### a2a unpublish

下架 agent（删除）。

```bash
p2pctl a2a unpublish --agent-id <id> [--json] [--data-dir <path>]
```

- `--agent-id`：agent ID
- `--json`：输出 JSON 格式
- `--data-dir`：数据目录（默认 ./p2p-data）

### a2a allow

授权 peer 访问 private agent（upsert：条目已存在则为更新并刷新 granted_at）。

```bash
p2pctl a2a allow --agent-id <id> --peer-id <peer> [--json] [--data-dir <path>]
```

- `--agent-id`：agent ID
- `--peer-id`：对端 PeerId（base58，32 字节）
- `--json`：输出 JSON 格式
- `--data-dir`：数据目录（默认 ./p2p-data）

### a2a disallow

撤销授权（删除条目；不存在明确报错不静默）。

```bash
p2pctl a2a disallow --agent-id <id> --peer-id <peer> [--json] [--data-dir <path>]
```

- `--agent-id`：agent ID
- `--peer-id`：对端 PeerId（base58，32 字节）
- `--json`：输出 JSON 格式
- `--data-dir`：数据目录（默认 ./p2p-data）
