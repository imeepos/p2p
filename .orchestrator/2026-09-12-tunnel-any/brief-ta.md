# 任务书 TA：tunnel 泛化契约冻结 + 访侧反代核心签名桩

类型：architecture | 预算：60 次工具调用，分段检查点回报（30 时报一次进度）|
分支：feat/wta-tunnel-contract | worktree：.worktrees/wta-contract（自建，基于 main @ a9ea63e1 之后最新 origin/main）

## 目标（一句话）

为「任意 HTTP/WS 服务经 P2P 分享」波次冻结全部契约面，并把访侧反代核心的
**可编译签名桩**落进 crates/p2p-tunnel（只签名不实现，GUI 零改动）。

## 背景（自包含）

p2p 仓的 /p2p-base/tunnel/1 协议已通用（被访侧 p2pctl tunnel serve 可分享任意
`127.0.0.1:<port>` 服务），但访侧反代核心（LocalProxy/head 头重写/pump，约 1400 行）
目前全部在 apps/gui/src-tauri/src/tunnel/（DSH 专用入口）。本波要把核心下沉
crates/p2p-tunnel、新增 CLI 访侧 tunnel connect、GUI 通用入口。你的卡是先行契约卡：
后续 TB（填实现+切 GUI）、TC（CLI connect）、TD（GUI 入口）三卡全部消费你冻结的
签名与契约条款，**你的签名就是它们的并行基础**。

先读：AGENTS.md（分支/提交纪律）→ 本文件全篇 → 指定源文件。

## 产出物（精确路径与内容）

1. `crates/p2p-tunnel/src/local_proxy.rs` — 可编译签名桩：
   - 从 `apps/gui/src-tauri/src/tunnel/{proxy.rs,head.rs,pump.rs}` 机械提炼公共面：
     `pub trait TunnelOpener`（参照 proxy.rs 现形状）、`pub struct LocalProxy` 及其
     `bind(...)`/`serve()`/`local_addr()`、head 模块的**纯函数签名**（Host/Origin/
     Referer 重写 + 请求头解析面）、pump 模块导出面。
   - 桩体一律 `todo!("W-TB")`；每个 pub 项带 doc 注释，写明语义出处（§3.3/现实现行号）。
   - **禁引入 tauri/tokio 之外的 GUI 依赖**；核心必须只依赖 p2p-tunnel 既有导出
     （TunnelIo/TunnelError/TunnelTicket 等）+ tokio + bytes/http 之类已workspace 既有依赖。
   - `lib.rs` 挂模块并 re-export。**禁改 GUI 任何文件**（切换是 TB 的事）。
2. `docs/design/gui-contract.md` §19 扩展（**独立小提交**，不与桩混提交）：
   - 新增 §19.9（编号顺延，先读现文确认）：`tunnel_open(target, peer)` 通用命令——
     参数 target=`127.0.0.1:<port>` 字面量（服务端校验，非字面量 Err）、
     peer=被访节点 PeerId；返回复用 TunnelOpenReport，**token 以空串承载
     「无 token」语义**（open_url=local_addr 本身，无 token 拼装）；事件复用
     tunnel_status。tunnel_open_dsh 保留不废弃（DSH 启动 URL 专用形态）。
   - §19.8 CLI 对等条款补一行：`p2pctl tunnel connect`（headless 访侧，TC 落地）
     对等 exempt（先例引 serve 条款措辞）。
3. `docs/protocol/specs/tunnel.md` §8 修正（与 2 同一独立小提交或再独立）：
   - 「实现状态」段：改为已实现口径 + 登记实现漂移与去向（反代核心现状 GUI 承载，
     本波下沉 crates/p2p-tunnel local_proxy 模块，迁移卡 TB）；冻结接口表 LocalProxy
     行保持 crates/p2p-tunnel 归属不变。**§1-§7 线格式语义一个字不改**（协议未变）。
4. `PROGRESS.md`（worktree 根）：分段进度 + **桩签名逐字清单**（每个 pub 项
   全签名），这是协调者复核与 TB 任务书的输入。

## 验收标准（可机械核验）

- `cargo check -p p2p-tunnel` EXIT=0；`cargo clippy -p p2p-tunnel --all-targets -- -D warnings` EXIT=0；
  `cargo fmt --check` 过（rustfmt 口径与 make fmt 一致）。
- 全量 `cargo check --workspace` EXIT=0（证明桩不破坏任何既有编译面；GUI 未切换
  所以 src-tauri 无新依赖）。
- gui-contract §19 新增条款含 tunnel_open 完整形状（参数/返回/事件/错误语义）与
  §19.8 connect 条款；tunnel.md §8 与实现现状一致。
- git log：桩提交与文档提交分离（文档可再拆，登记类独立小提交）。
- PROGRESS.md 含逐字签名表。

## 边界（明确不做）

- 不填实现（todo!() 保留）；不改 apps/gui、apps/cli、crates/p2p-itest；
  不动 registry.toml（协议 ID 与状态不变）；不改 §1-§7 线格式语义；
  不做 CLI/GUI 命令实现。

## 停止条件（立即报 BLOCKED）

提炼中发现：LocalProxy/head 签名依赖 tauri 或 GUI 内部类型而无法在 p2p-tunnel
表达；或现实现与 §19/specs 存在实质冲突。回报格式：DONE / DONE_WITH_CONCERNS /
BLOCKED / NEEDS_CONTEXT + 行为证据（命令+退出码）+ 结构化复盘
（做对了/踩坑/给下张卡的移交清单）。完成后 push origin 分支，**禁自合并**，
session_link_send_parent 回报派发者。

## 环境注意

- cargo 在 ~/.cargo/bin（不在默认 PATH 先 export）。
- 提交多行 message 用 `git commit -F -`，禁 `$(cat <<EOF)`；只 add 显式路径。
- 已知坑（来自仓内踩坑记录）：跨 crate 类型归属先 grep 目标 lib.rs re-export 面
  再写 import；管道收尾吃退出码——验证命令不带管道或 set -o pipefail。
