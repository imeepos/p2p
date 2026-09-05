# p2pctl 使用指南（CLI 对等手册）

p2pctl 是 p2p-console（GUI）的等价命令行入口。项目裁定：**GUI 的所有操作都必须有等价
CLI**，由守卫 `scripts/check/cli-parity.sh` 在 `make check` 中机械化执行（见 §8）。

## 1. 构建

仓库无预编译产物，从源码构建（产物在 `apps/cli/target/debug/p2pctl`）：

```bash
export PATH=$HOME/.cargo/bin:$PATH   # macOS 默认 cargo 不在 PATH，缺了报 command not found
cargo build --manifest-path apps/cli/Cargo.toml
```

下文以 `p2pctl` 指代该二进制路径。clippy 与单测随主仓门禁约定执行：

```bash
cargo clippy --manifest-path apps/cli/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/cli/Cargo.toml
```

## 2. 退出码约定

| 退出码 | 含义 |
|---|---|
| 0 | 成功（含幂等类"无事可做"，如重复 stop、clear 已空的日志） |
| 1 | 运行失败（节点未运行、白名单拒绝、网络失败等，错误信息走 stderr，前缀 `p2pctl: `） |
| 2 | 用法错误（参数缺失/非法，由 clap 报告） |

## 3. --json 约定

每个读命令默认输出人读文本；加 `--json` 输出同源结构化 JSON（同一事实源的两种渲染）。
文本形态刻意带 `key=value` 行（如 `pid=42`、`activeConnections=0`），供脚本无 JSON
依赖地 grep 采集；JSON 字段名 camelCase，与 GUI 契约字段逐字同形。

## 4. 数据目录约定

- CLI 的应用数据目录等价于 GUI app 数据目录：默认 `./p2p-data`，全局可用
  `--data-dir <dir>` 覆盖。目录内文件名与 GUI 一致（`gui-config.json`、
  `node-profile.json`），指向同一目录即操作同一份数据。
- 节点身份数据目录取配置 `dataDir`，缺省回落 `<data-dir>/p2p-data`（对齐 GUI 兜底）。
- 运行时可观测信号（守护进程）：`daemon.pid` / `daemon.meta.json` / `daemon.sock` /
  `daemon.log`。
- **GUI 日志目录**（`log` 域读取对象）：即 Tauri `app_log_dir`，identifier
  `com.p2p.console`。macOS 为 `~/Library/Logs/com.p2p.console`，Linux 在
  `$XDG_STATE_HOME`（缺省 `~/.local/state`）下 `com.p2p.console/logs`，Windows 在
  `%LOCALAPPDATA%\com.p2p.console\logs`。`log` 域可用 `--log-dir` 覆盖（测试/E2E 用）。

## 5. 命令域清单

### node —— 节点域

```bash
p2pctl node status  [--data-dir DIR] [--json]   # 查询运行状态（pid/peer/addr/log 行）
p2pctl node start   [--data-dir DIR] [--json]   # 启动守护进程（读 gui-config.json）
p2pctl node stop    [--data-dir DIR] [--json]   # 停止（幂等，重复 stop 退出码 0）
p2pctl node log tail [--lines N] [--data-dir DIR] [--json]  # daemon.log 尾读（默认 200、上限 1000，文件缺失不算错）
```

`node log tail` 读的是节点守护进程日志 `<data-dir>/daemon.log`（与 GUI 前端
日志 frontend.log 分属两路，见 log 域说明）。`node start --json` 输出经写后
flush 落盘，重定向到文件不会丢内容（脚本/CI 直接读重定向文件取 peerId/addr）。

### chat —— 聊天域（对齐 GUI 契约 §12）

```bash
p2pctl chat friends list                                  # 好友簿列表
p2pctl chat friends add <PEER_ID> [--nickname N] [--addr A] [--note X]  # 发好友邀请（对方同意后互为好友）
p2pctl chat friends invites list|accept|reject|cancel           # 邀请生命周期（写命令同支持 --json）
p2pctl chat friends remove <PEER>                         # 幂等
p2pctl chat friends update <PEER> [--group G] [--nickname N] [--note X] [--addr A ...]  # 补丁修改（--addr 可重复，整组替换）
p2pctl chat history --peer <PEER> [--limit N] [--data-dir DIR] [--json]  # 消息历史（time desc）
p2pctl chat send --peer <PEER> --text "..."               # 文本消息（离线排队 status=pending 退出 0）
p2pctl chat send --peer <PEER> --file <PATH>              # 附件消息（mime 按扩展名嗅探）
p2pctl chat outbox list [--data-dir DIR] [--json]         # 行箱按对端列积压与已投计数
p2pctl chat outbox flush [--peer PEER] [--data-dir DIR]   # 手动补投积压（逐对端回报结果）
p2pctl chat media file --message-id <ID> [--data-dir DIR] # 查附件落盘绝对路径
p2pctl chat serve [--data-dir DIR] [--quic-port PORT]     # 常驻聊天节点（端口记忆：重启沿用上次端口）
```

注意：聊天收发用 `chat serve` 输出的 **chat 身份** peerId（与 `node start` 守护
peerId 不同根）；同数据目录 `chat serve` 与 `chat send` 经 `identity.lock` 互斥
不可并存。对端离线时 `chat send` 排队（status=pending，退出码 0、
delivered=false），对端恢复可达后由 serve 启动/周期补投泵自动补投，或经
`chat outbox flush` 手动补投；对端换端口后好友簿旧地址由入站帧自学习回写，
也可 `chat friends update --addr` 手工修正。两节点最小拓扑配方与失败形态见
`docs/ops/p2pctl-ai-guide.md` 附录A。可达性闭环 E2E：
`bash scripts/ops/cli-chat-reach-e2e.sh`（末行 PR1-REACH-OK，Not in make check）。

### config / profile —— 配置与资料域

```bash
p2pctl config get  [--data-dir DIR] [--json]   # 无文件输出 GUI 出厂默认值
p2pctl config save [FILE] [--data-dir DIR]     # FILE 为 "-" 或省略读 stdin（完整 JSON）
p2pctl profile get  [--data-dir DIR] [--json]
p2pctl profile save [FILE] [--data-dir DIR]
```

### peer —— 对端域

```bash
p2pctl peer dial "<PEER_ID>@<ADDR>" [--data-dir DIR]  # 登记地址并连接；ADDR 为 ip/u端口 或 ip/t端口
p2pctl peer connect <PEER_ID> [--data-dir DIR]        # 按地址簿直连
p2pctl peer disconnect <PEER_ID> [--data-dir DIR]     # 幂等挂断
p2pctl peer ping <PEER_ID> [--timeout-ms MS]          # echo 协议测 RTT（rtt_ms= 行；<1ms 以 0.1ms 精度并附 rtt_us）
p2pctl peer list [--data-dir DIR] [--json]            # 地址簿 + 在线态（首行 total/connected，逐对端 peer=/addr= 行）
```

`peer dial` 输出的 hops 为降级链逐跳报告：direct=按地址簿直连尝试；
punch=经中继信令打洞；relay=中继电路兜底；hops=[] 表示复用池内已有
连接（本次未发生新拨号，不代表无路径）。

### discovery —— 发现域（观测只读）

```bash
p2pctl discovery list [--data-dir DIR] [--json]  # 地址缓存（邻居与登记地址）+ 来源计数（口径同 GUI 发现页）
```

### relay —— 中继域（观测只读）

```bash
p2pctl relay status [--data-dir DIR] [--json]  # 中继会话/重连水位 + 降级链逐跳统计 + 配置端点
```

两命令事实源为守护进程内观测注册表（订阅节点事件聚合，语义同 GUI
邻居表），因此只反映本守护进程启动后的痕迹；节点未启动报错退出码 1。

### identity —— 身份域

```bash
p2pctl identity init [--data-dir DIR] [--json]  # 显式创建本机节点身份（0600 落盘；幂等，已存在输出既有身份退出 0）
p2pctl identity show [--domain node|chat] [--data-dir DIR] [--json]  # 只读查身份：不起进程、不占 identity.lock
p2pctl identity reset --confirm [--data-dir DIR]  # 危险操作：停节点 + 删 key.seed；缺 --confirm 拒绝
```

注意：node 身份在 `<data-dir>/p2p-data/key.seed`（守护身份），chat 身份在
`<data-dir>/key.seed`（聊天身份，同 `chat serve` 首行输出的 peerId），两根不同。
`identity show --domain chat` 可离线读聊天身份，无需起 serve、不触发 `identity.lock` 互斥。

### log —— GUI 前端日志域（CL4）

对应 GUI 设置页"前端日志"三操作。读取对象是 GUI 日志目录下的 `frontend.log`
（GUI 把浏览器侧 error/unhandledrejection/console.error 以 JSONL 落盘于此）。

```bash
p2pctl log tail  [--lines N] [--log-dir DIR] [--data-dir DIR] [--json]  # 末尾 N 行，默认 200、上限 1000（同 GUI）
p2pctl log path  [--log-dir DIR] [--data-dir DIR] [--json]              # frontend.log 绝对路径
p2pctl log clear [--log-dir DIR] [--data-dir DIR] [--json]              # 删 frontend.log 与 frontend.log.1，幂等
```

`--data-dir` 是 `--log-dir` 的同义别名（F7 参数命名对齐：他域均用
--data-dir），语义不变：读 `<DIR>/frontend.log`；两者同时给出时
`--data-dir` 优先。

注意：GUI 进程自身日志为同目录 `p2p-console.log`（tracing 落盘），节点守护进程日志为
`<data-dir>/daemon.log`；两者不属于 frontend.log 语义，不在 `log` 域范围内。

### metrics —— 运行时指标域（CL4）

```bash
p2pctl metrics get [--data-dir DIR] [--json]   # 指标快照；未运行返回全零（同 GUI 渲染语义）
```

字段：`dialDirectOk/Fail`、`dialPunchOk/Fail`、`dialRelayOk/Fail`、
`addrDialFailures`、`relayReconnects`、`gateDenialsTotal`、`activeConnections`、
`relaySessionsActive`。运行中经 `daemon.sock` 控制通道取实时快照。

### update —— 更新域（CL4）

```bash
p2pctl update check [--json]   # 查 GitHub 最新稳定 release，与当前版本比较（10s 超时）
p2pctl update open [--url URL] [--json]
```

- `update check`：拉取 Releases API，仅取稳定候选（排除 draft/prerelease/非三段 tag），
  输出 `current= / latest= / hasUpdate= / url= / name=` 行；无候选时 `latest=（无稳定
  release 候选）`。
- `update open`：**CLI 语义为输出 release 页 URL，不开浏览器**（对应 GUI
  `update_open_release_page` 的映射差异，已登记映射表）。带 `--url` 时仅做白名单校验
  （https 且 host 恰为 github.com）后原样输出；不带 `--url` 时先执行检查取最新候选。

### gui —— GUI 控制通道域（GC2）

操作**运行中的 GUI**（非节点）：对接 GUI 本地控制通道（127.0.0.1 HTTP JSON +
token，契约见 `docs/design/gui-control-channel.md`）。通道发现：读 GUI 数据目录
`control/endpoint.json`（pid 探活甄别崩溃残留）+ `control/token`；GUI 未运行、
端点文件缺失或进程已死均结构化报错（含「请先启动 GUI」指引，退出码 1）。
macOS 数据目录：`~/Library/Application Support/com.p2p.console/control/`，
可用 `--gui-data-dir DIR` 覆盖（测试/E2E 用）。

```bash
p2pctl gui status [--gui-data-dir DIR] [--json]          # 版本/窗口/当前路由/pid/录制态
p2pctl gui screenshot -o <绝对路径> [--gui-data-dir DIR] [--json]   # 主窗口 PNG 原子落盘
p2pctl gui record start -o <绝对路径> [--interval-ms MS] [--gui-data-dir DIR] [--json]  # GIF 录屏
p2pctl gui record stop  [--gui-data-dir DIR] [--json]    # 收尾落盘，回报 path/frames/bytes
p2pctl gui navigate <路由> [--gui-data-dir DIR] [--json] # dashboard/peers/discovery/relay/chat/events/settings/diagnostics
p2pctl gui invoke <命令> [--arg k=v ...] [--gui-data-dir DIR] [--json]  # 白名单只读转发
p2pctl gui page [--gui-data-dir DIR] [--json]             # 当前页语义 descriptor（--json 含 args schema 与 state）
p2pctl gui action <页面> <动作> [K=V...] [--navigate] [--gui-data-dir DIR] [--json]  # 执行页面动作
```

- `invoke` 白名单由 GUI 侧权威维护（当前：node_status / metrics_get /
  metrics_history / config_get / profile_get），越权命令返回 `INVOKE_FORBIDDEN`
  退出码 1；`--arg k=v` 中 v 可解析为 JSON 值则保留类型。
- `page`/`action` 消费页面语义协议（GET page/current / POST page/action，GC3 §9）：
  页面注册表由 GUI 前端维护（示范页 chat/peers/settings，其余页随 GC3b 扩量），
  动作与页面按钮同源（store/IPC），非 DOM 模拟。`page` 文本输出 page/schemaVersion/
  name/description/actions 表格（每动作含 args schema 标注与 [confirm] 标记），
  `--json` 为服务端全量 {schemaVersion,page,descriptor}。
- `action` 的 K=V 参数布尔/数字按 JSON 类型解析（与 `--arg` 同规则）；requestId 由
  CLI 自动生成（`cli-<pid>-<seq>`）；非当前页默认结构化报错并附「gui navigate <页面>」
  指引，`--navigate` 先切页再执行；危险动作（descriptor 标 confirm）缺 `confirm=true`
  时服务端以 `ACTION_CONFIRM_REQUIRED` 拒绝，CLI 透传错误码可读呈现；回包为
  `{requestId,result}`，文本渲染 result，`--json` 全量。
- screenshot/record 依赖 macOS 屏幕录制权限：权限缺失时 GUI 返回
  `CAPTURE_PERMISSION_DENIED`，CLI 原样透出（不静默、不重试）；GUI 重编译后 TCC
  授权记录可能失效，需在 系统设置 > 隐私与安全性 > 屏幕录制 重新授权后重试。
- 该域为 CLI 单侧能力（GUI 命令面未新增 Tauri 命令），不进 §6 映射表。

## 6. GUI 命令映射表

权威机器可读版本：`scripts/check/cli-parity.tsv`（守卫消费，勿手工漂移）。人读版：

| GUI 命令 | CLI | 说明 |
|---|---|---|
| node_start / node_stop / node_status | node start / stop / status | |
| config_get / config_save | config get / save | |
| profile_get / profile_save | profile get / save | |
| peer_dial / peer_connect / peer_disconnect / peer_ping | peer dial / connect / disconnect / ping | |
| identity_reset | identity reset | |
| metrics_get | metrics get | CL4 补齐 |
| metrics_history | （豁免，待裁决） | GUI 为 5s 采样 120 点环形历史；CLI 等价需守护进程新增采样行为（非薄封装），CL4 不扩权，已回报待裁决 |
| frontend_log_append | （豁免） | GUI 前端专属行为：浏览器侧采集源，CLI 无浏览器运行时，提供对等子命令无意义 |
| frontend_log_tail / path / clear | log tail / path / clear | |
| update_check | update check | |
| update_open_release_page | update open | CLI 不开浏览器，输出 URL |
| chat_friends_list / chat_friend_add / chat_friend_remove | chat friends list / add / remove | |
| chat_history / chat_send / chat_media_file | chat history / send / media file | |

## 7. E2E 脚本

均自动构建缺失的二进制、临时目录隔离、trap 清理造数，可重复执行：

```bash
bash scripts/ops/cli-node-e2e.sh        # CL2：node/config/peer/identity 全链路（末行 CL2-E2E-OK）
bash scripts/ops/cli-chat-e2e.sh        # CL3：chat 好友/历史/发送/附件双节点（末行 CL3-E2E-OK）
bash scripts/ops/cli-log-update-e2e.sh  # CL4：log/metrics/update 域（末行 CL4-E2E-OK）
bash scripts/ops/cli-gui-e2e.sh        # GC2：gui 域 × 真实 GUI 控制通道（末行 GC2-E2E-OK）
bash scripts/ops/cli-gui-data-e2e.sh   # N2：GUI×CLI 数据面互操作（末行 N2-E2E-OK）
bash scripts/ops/cli-page-e2e.sh       # GC4：gui page/action × 真实 GUI 页面协议 + 前后截图证据（末行 GC4-E2E-OK）
bash scripts/ops/cli-live-e2e.sh       # W1：CLI 写入 → GUI 实时感知 file-watch（末行 W1-E2E-OK）
bash scripts/ops/cli-observe-e2e.sh    # PR2：观测对等六语义（重定向/log tail/peer list/discovery/relay/--data-dir 别名，末行 PR2-OBSERVE-OK）
```

`cli-live-e2e.sh`（W1 实测）：隔离 HOME 启动真实 GUI（custom-protocol 特性内嵌
产物，免疫外部 vite dev server 占用 devUrl 端口），前端感知链路就绪门
（frontend.log 的 data-watch-ready）后 CLI 运行中写 config / 好友簿，≤3s 内
断言 GUI 侧经 data-changed 定向重载（frontend.log 感知证据行），并以 invoke
白名单只读命令读回与 CLI 写入逐字段一致；两连绿为验收口径。

`cli-gui-data-e2e.sh` 以临时 HOME 隔离启动真实 GUI，与 CLI 指向同一数据目录：
CLI 冷写 config/profile/chat → invoke 白名单读回断言一致 → 运行中再写实测感知
语义 → R2 并发写子脚本 `cli-chat-concurrency-e2e.sh`（末行 N2-R2-OK）。
一致性语义结论见 §9。

`cli-gui-e2e.sh` 会构建并后台启动**真实 GUI**，轮询端点状态文件就绪后走全链路
断言；已有 GUI 实例运行时先备份 `endpoint.json`、以 pid 匹配本实例、退出后还原，
不影响既有实例；screenshot/record 权限失败时输出屏幕录制权限提示后再失败（可见）。

## 8. 对等守卫

`make check` 聚合了 `scripts/check/cli-parity.sh`：

1. 机械提取 `apps/gui/src-tauri/src/lib.rs` 中 `generate_handler![...]` 的命令全集；
2. 对照 `scripts/check/cli-parity.tsv` 映射表（列：gui_command / mapped|exempt /
   cli_invocation / reason，豁免必填理由）；
3. mapped 行用 `p2pctl --help` 输出实测存在性（递归到叶子子命令），禁止只对表不验命令；
4. 缺映射、映射命令不存在、豁免无理由、表内陈旧行 → 列出清单并以非 0 退出；
5. 通过时末行输出 `CLI-PARITY-OK`。

新增 GUI 命令时必须同步登记映射表（mapped 或带理由的 exempt），否则 `make check` 必红。
守卫自身有正反夹具自测（`scripts/check/tests/cli-parity.sh`，随 `gate-tests` 运行）。

## 9. 数据面一致性语义（N2 实测）

GUI（apps/gui）与 CLI（p2pctl）共用同一数据目录：CLI `--data-dir` 即 GUI app
数据目录（macOS `~/Library/Application Support/com.p2p.console`），其下
`gui-config.json`、`node-profile.json`、`chat/`（好友簿与消息库）为两端口径
同一份文件；控制通道状态在 `control/` 子目录。实测脚本：
`scripts/ops/cli-gui-data-e2e.sh`（末行 N2-E2E-OK）、
`scripts/ops/cli-live-e2e.sh`（末行 W1-E2E-OK，实时感知升级见 9.2/9.5）。

### 9.1 冷启动一致性（实测）

GUI 启动即按目录约定读盘：先 CLI 写 `config save` / `profile save` /
`chat friends add`，再启动 GUI，经 `p2pctl gui invoke` 白名单只读命令
（config_get / profile_get）读回，与 CLI 读路径 JSON 深比较逐字段一致；
chat 好友簿以 CLI 读路径与磁盘文件比对验证（见 9.3 缺口）。

### 9.2 运行中实时性（W1 实测升级 + 实测结论）

W1 起 GUI 内置数据目录监听（notify + debouncer，防抖 500ms，非递归 +
白名单归类防递归风暴），CLI 写入关键文件后向前端发
`data-changed{domains}`，前端单监听器按域定向重载（禁全应用重载）；
GUI 自身写盘以写序号窗口（2s）做回声抑制，不重复重载。实测
（`cli-live-e2e.sh`，末行 W1-E2E-OK）：config 与好友簿写入 GUI 侧
感知时延 1s（≤3s 契约）。

| GUI 侧读路径 | CLI 写入后的感知 | 机理 |
|---|---|---|
| invoke config_get / profile_get | 即时可见，无需刷新或重启 | 每次调用直读磁盘文件 |
| node_status 的 config 字段 | 节点运行中为启动时快照 | RunningNode 缓存启动配置，需节点重启生效（读码，W1 未变） |
| GUI 前端页面（设置资料/发现/中继/聊天好友簿） | 自动定向刷新（≤3s，实测 1s） | watcher → data-changed → 前端按域重载（W1 升级；发现/中继页未保存的本地编辑态 localConfig 不被外部写入覆盖，仍以本地为准） |
| GUI chat 视图好友簿 | 自动定向刷新（节点运行中） | chat_friends_list 依赖运行中节点，节点未启动时跳过重载（W1） |
| 降级语义（R3） | watcher 初始化失败不阻断 GUI | 结构化错误记日志 + data-watch-status{active:false}，前端置降级态可判；外部写入需手动刷新 |

### 9.3 白名单缺口（R4 回报项，不阻断）

invoke 白名单刻意只收只读命令（`apps/gui/src-tauri/src/control/invoke_allow.rs`
红线：写操作永不入列），config_save / profile_save / chat_friend_add 等写命令
一律 `INVOKE_FORBIDDEN`。因此「GUI 写 → CLI 读回」无法经控制通道驱动验证：
需 GUI 前端手动操作，或另立卡评估「可观察写命令入白名单」；E2E 以拒绝断言
记录该缺口。chat 只读命令（chat_friends_list）同样不在白名单，GUI 侧 chat
视图一致性暂无 CLI 观测面。

### 9.4 并发写语义（实测 + 读码，子脚本末行 N2-R2-OK）

- `chat/friends.json`（Y1 起，yrs CRDT 承载）：好友簿由 yrs（Yjs 官方 Rust 移植）
  Doc 承载为逐行更新日志——首行为格式头 `{"p2p-friends":"yrs-v1"}`，其余每行为
  一次实际变更的 base64 yrs update（O_APPEND 追加）。yrs update 幂等可交换，
  双进程并发 add/remove **无需文件锁**即全量保留（每次操作前以磁盘日志重建
  权威态再合并，CRDT 合并语义取代原文件锁串行化），remove 走 yrs tombstone，
  并发 add/remove 按 yrs 语义合并；同 peer 并发改写由 yrs 确定性裁决。旧 JSON
  数组首次载入自动迁移，原文件备份为 `friends.json.bak-yrs-<ts>`；store 对外
  API（add/remove/list）签名与语义不变，CLI/GUI 无感。每行日志解析失败跳过
  并 warn（可观测），文件名沿用 `friends.json`（watcher 归类不变）。消息 JSONL
  不在 CRDT 范围（append-only 已行级完整）；好友簿日志随变更笔数线性增长，
  compaction 留待 Y2 决策。
- `chat/messages/`、`chat/outbox/`：JSONL 追加式（O_APPEND 行级写入），状态
  改写走整文件重写并原样保留未知行；双流并发各发 N 笔（各 peer 一文件）实测
  恰 N 条、id 唯一、CLI history 与磁盘逐条一致、messages/ 目录无孤儿文件。
  同 peer 跨进程高频并发追加存在行间交错的理论窗口（读取时跳过损坏行并
  warn），属存储层已知边界，未纳入断言。

### 9.5 实时感知链路（W1，实测末行 W1-E2E-OK）

链路：CLI 原子写 → notify（非递归挂载 app 数据目录与 `chat/` 目录）→
debouncer 防抖 500ms 归并 → 白名单归类（`gui-config.json` /
`node-profile.json` / `chat/friends.json`，原子写 tmp 同前缀归同域）→
`data-changed{domains}` → 前端单监听器按域分发注册的定向重载器 →
感知证据行落 `frontend.log`（`{"kind":"data-changed","domains":[…],
"ts":…}`，E2E 与排障直接读文件）。边界与口径：

- GUI 自身写盘同样触发事件，前端以 `markLocalWrite` 写序号窗口（2s，覆盖
  写盘→防抖→回流全链路）按域回声抑制，不重复重载；窗口外事件正常生效。
- 好友簿目录由 GUI 启动期预建，消除「建目录 + 首写同瞬间」懒挂载丢事件
  窗口；运行中目录被删后重建由目录事件补挂载。
- E2E 的 GUI 以 `custom-protocol` 特性构建（内嵌 frontendDist）：debug 二进制
  默认加载 devUrl(localhost:5173)，外部 vite dev server 占用端口会让 GUI 装
  上旧 dev bundle（W1 开发中实证的假红源）；日常 `tauri dev` 不带此特性，
  行为不变。
- 感知证据行的前端观测面为 frontend.log；删除文件再写入等极端变更序列由
  防抖按路径归并，域内不放大。

