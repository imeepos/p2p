# p2pctl

GUI 命令面的等价 CLI（对等裁定：GUI 上所有操作都必须有等价 CLI 操作）。
独立 cargo 项目，不参与根 workspace；经路径依赖复用 `crates/p2p` facade、
`crates/p2p-cli`（echo 协议）与 `crates/p2p-log`（守护进程日志）。

## 约定（后续波次遵守）

- 每个命令域一个模块文件（`src/node.rs`、`src/config.rs`…），域内自带子命令枚举与执行入口。
- 子命令注册点集中唯一：`src/cli.rs` 的 `Command` 枚举。
- 默认输出人读文本（机器可 grep 的 `key=value` 行）；`--json` 开关输出结构化 JSON
  （camelCase，与 GUI 契约同源），可放在子命令末尾。
- 退出码：0 成功、1 运行失败、2 用法错误（`src/error.rs`）。

## 数据目录（R4）

`--data-dir`（默认 `./p2p-data`）是 CLI 的 app 数据目录等价物，与 GUI 同一文件约定：
`gui-config.json`、`node-profile.json`，节点身份数据默认在其下 `p2p-data/`（key.seed）。
守护进程可观测信号：`daemon.pid`、`daemon.meta.json`（peer/监听地址）、
`daemon.log`（p2p-log 落盘）、`daemon.sock`（控制通道）。

## 守护进程模型（node 域）

`node start` 拉起 `node serve` 守护进程（持有 facade Node + echo handler），
经 `daemon.sock` 提供 JSON 行协议控制面（status/dial/connect/disconnect/ping）；
`node stop` 以 SIGTERM 优雅停机（超时 SIGKILL 兜底），重复 stop 幂等返回 0。
`peer dial/connect/disconnect/ping` 语义与 GUI 同名 Tauri 命令一致，
由守护进程代执行（节点未启动报错、退出码 1）。

## 命令域波次

- CL1：脚手架 + `node status` 纵切。
- CL2（本波）：`node start/stop/status`、`config get/save`、`profile get/save`、
  `peer dial/connect/disconnect/ping`、`identity reset`（须 `--confirm`）。
- CL3：chat。CL4：logs/update + CLI-GUI 对等守卫。

## 使用

```sh
cargo build --manifest-path apps/cli/Cargo.toml
apps/cli/target/debug/p2pctl --version

# 节点生命周期（--data-dir 全程隔离）
apps/cli/target/debug/p2pctl node start --data-dir /tmp/n1
apps/cli/target/debug/p2pctl node status --data-dir /tmp/n1 --json
apps/cli/target/debug/p2pctl node stop  --data-dir /tmp/n1

# 配置与资料（空态输出默认值，对齐 GUI 首跑）
apps/cli/target/debug/p2pctl config get --data-dir /tmp/n1 --json
apps/cli/target/debug/p2pctl config save --data-dir /tmp/n1 --config "$(cat gui-config.json)"
apps/cli/target/debug/p2pctl profile save --data-dir /tmp/n1 '{"name":"家用节点"}'

# 对端（先启动节点，语义同 GUI peer_* 命令）
apps/cli/target/debug/p2pctl peer dial "<peer_id>@<ip>/u<port>" --data-dir /tmp/n1
apps/cli/target/debug/p2pctl peer ping "<peer_id>" --data-dir /tmp/n1
apps/cli/target/debug/p2pctl peer disconnect "<peer_id>" --data-dir /tmp/n1

# 身份重置（危险操作，必须显式确认）
apps/cli/target/debug/p2pctl identity reset --confirm --data-dir /tmp/n1

# E2E（双节点拨号测距全链路，临时目录隔离，末行 CL2-E2E-OK）
bash scripts/ops/cli-node-e2e.sh
```

## 空态语义（R2）

全新数据目录下 `config get` / `profile get` 输出厂默认值（内置云端端点、
mDNS 开启、dataDir=<data-dir>/p2p-data），退出码 0；配置损坏回退默认并留 stderr 告警。
