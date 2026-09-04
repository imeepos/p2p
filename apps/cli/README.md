# p2pctl

GUI 命令面的等价 CLI（对等裁定：GUI 上所有操作都必须有等价 CLI 操作）。
独立 cargo 项目，不参与根 workspace；经路径依赖复用 `crates/p2p-cli` 与底座 crate。

## 约定（后续波次遵守）

- 每个命令域一个模块文件（`src/node.rs`、`src/config.rs`…），域内自带子命令枚举与执行入口。
- 子命令注册点集中唯一：`src/cli.rs` 的 `Command` 枚举。
- 默认输出人读文本；`--json` 开关输出结构化 JSON，可放在子命令末尾。
- 退出码：0 成功、1 运行失败、2 用法错误（`src/error.rs`）。

## 命令域波次

- CL1（本波）：脚手架 + `node status` 纵切。
- CL2：node/config/profile/peer/identity。CL3：chat。CL4：logs/update + CLI-GUI 对等守卫。

## 使用

```sh
cargo build --manifest-path apps/cli/Cargo.toml
apps/cli/target/debug/p2pctl --version
apps/cli/target/debug/p2pctl node status [--json] [--data-dir ./p2p-data]
```
