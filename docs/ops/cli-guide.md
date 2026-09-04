# p2pctl 使用指南（CLI 对等手册）

p2pctl 是 p2p-console（GUI）的等价命令行入口。项目裁定：**GUI 的所有操作都必须有等价
CLI**，由守卫 `scripts/check/cli-parity.sh` 在 `make check` 中机械化执行（见 §8）。

## 1. 构建

仓库无预编译产物，从源码构建（产物在 `apps/cli/target/debug/p2pctl`）：

```bash
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
```

### chat —— 聊天域（对齐 GUI 契约 §12）

```bash
p2pctl chat friends list                                  # 好友簿列表
p2pctl chat friends add   --peer-id <PEER> [--name N] [--note X]   # upsert 幂等
p2pctl chat friends remove <PEER>                         # 幂等
p2pctl chat history <PEER> [--limit N] [--data-dir DIR] [--json]  # 消息历史（time desc）
p2pctl chat send --peer-id <PEER> --text "..."            # 文本消息
p2pctl chat send --peer-id <PEER> --file <PATH>           # 附件消息
p2pctl chat media file --message-id <ID> [--data-dir DIR] # 查附件落盘绝对路径
p2pctl chat serve [--data-dir DIR]                        # 常驻聊天节点（E2E/守护支撑）
```

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
p2pctl peer ping <PEER_ID> [--timeout-ms MS]          # echo 协议测 RTT（rtt_ms= 行）
```

### identity —— 身份域

```bash
p2pctl identity reset --confirm [--data-dir DIR]  # 危险操作：停节点 + 删 key.seed；缺 --confirm 拒绝
```

### log —— GUI 前端日志域（CL4）

对应 GUI 设置页"前端日志"三操作。读取对象是 GUI 日志目录下的 `frontend.log`
（GUI 把浏览器侧 error/unhandledrejection/console.error 以 JSONL 落盘于此）。

```bash
p2pctl log tail  [--lines N] [--log-dir DIR] [--json]  # 末尾 N 行，默认 200、上限 1000（同 GUI）
p2pctl log path  [--log-dir DIR] [--json]              # frontend.log 绝对路径
p2pctl log clear [--log-dir DIR] [--json]              # 删 frontend.log 与 frontend.log.1，幂等
```

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
```

- `invoke` 白名单由 GUI 侧权威维护（当前：node_status / metrics_get /
  metrics_history / config_get / profile_get），越权命令返回 `INVOKE_FORBIDDEN`
  退出码 1；`--arg k=v` 中 v 可解析为 JSON 值则保留类型。
- screenshot/record 依赖 macOS 屏幕录制权限：权限缺失时 GUI 返回
  `CAPTURE_PERMISSION_DENIED`，CLI 原样透出（不静默、不重试）。
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
```

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
