# 提案：tunnel 泛化波——任意 HTTP/WS 服务经 P2P 分享（2026-09-12）

## 用户需求

通过 p2p 可以分享任何 http/ws 等服务给另一个节点使用。

## 现状事实（Facts，已核实）

- F1 协议 /p2p-base/tunnel/1 已全通用：TunnelTicket.target 任意 `127.0.0.1:<port>`、
  被访侧 allowlist 显式白名单、数据面纯字节泵（WS 升级透传，W-T3c 实测 WS 101）。
  **协议与 registry 本波零改动。**
- F2 被访侧（分享方）已就绪：CLI `p2pctl tunnel serve --target`（多目标）；GUI
  `tunnel_serve_start(target)` 单目标。任意本地回环 HTTP/WS 服务都可作为被分享端。
- F3 访侧反代核心（LocalProxy/head 重写/pump，约 1400 行）全部在
  apps/gui/src-tauri/src/tunnel/，核心已参数化（target.port），DSH 耦合仅在
  url.rs（DshTarget=启动 URL+token）与命令命名层。
- F4 CLI 无访侧命令（只有 serve）——headless 节点无法作为使用方。
- F5 规范页 §8 冻结清单本就写 `LocalProxy | crates/p2p-tunnel`，实现漂移在 GUI，
  §8「实现状态」段仍写 planned（滞后未更新）。
- F6 GUI 访侧入口 tunnel_open_dsh 语义 = DSH 启动 URL（token 参与 open_url），
  前端 remote-access 页 DSH 专用。

## 差距 = 三件

1. 反代核心下沉 crates/p2p-tunnel（消除 F5 漂移，CLI/GUI 复用同一实现）。
2. CLI 访侧 `p2pctl tunnel connect`（headless 使用方，对称 W-T5）。
3. GUI 访侧通用入口（任意 target，非 DSH 专用）。

## 任务分解与依赖图

```
TA 契约+签名桩(architecture, 先行, 60)
  └→ TB 下沉实装+GUI 切换(code-quality, 120) ──┐
  └→ TC CLI connect+itest(code-quality, 120) ──┤ (TB∥TC 文件域不叠)
                                              ├→ TD GUI 通用入口(frontend, 120)
                                              └→ TE e2e 证据(code-quality, 100, TC+TD 合后)
```

- TA：crates/p2p-tunnel 新模块 local_proxy.rs **可编译签名桩**（todo!() 体，GUI 零
  切换）+ gui-contract §19 扩展（tunnel_open 通用命令 + §19.8 CLI connect 对等）+
  specs/tunnel.md §8 修正。桩签名逐字回报，协调者复核后合并。
- TB：桩填实现（GUI proxy.rs/head.rs/pump.rs/proxy_tests.rs 迁入）+ GUI 切换消费 +
  DSH 面（url.rs/token/audit）留 GUI 层。红绿护栏 = 迁移前后全部既有测试绿。
- TC：apps/cli tunnel connect（形态对齐 serve.rs：前台常驻、ready/stopped JSON 行、
  SIGTERM 优雅收口）+ 双节点 itest（HTTP GET + WS echo 全链）+ ai-guide/ai-docs-sync。
- TD：remote-access 页通用入口（target+peer 输入）+ src-tauri tunnel_open 命令薄层 +
  cli-parity（tunnel_open 映射、connect exempt 引 §19.8）+ 前端三态 + 截图链 + gui-check。
- TE：复用 wt3c 注入链 + wt5 102 拓扑：102 起真实 HTTP+WS 服务 + headless serve，
  mac 访侧全链证据（截图 + WS 双向帧 + 双面审计对账）。

## 契约裁定（协调者冻结，随任务书逐字下发）

- Ruling: 桩模块名 local_proxy.rs，类型/函数名沿用 GUI 现名（LocalProxy、
  TunnelOpener）最小漂移 — 依据：F3 已参数化、规范页 §8 已用此名 — 代价：若提炼中
  发现命名冲突，BLOCKED 回报不得自行改义。
- Ruling: tunnel_open 返回复用 TunnelOpenReport，token 以空串承载「无 token」语义
  （既有字段类型不变=加法兼容），文档写明 — 代价：空串是弱语义，但避免破坏性改类型。
- Ruling: DSH 语义（启动 URL 解析、token、open_url 拼装）永久留 GUI 层，不下沉 —
  依据：core 已参数化，下沉 DSH 概念会污染底座 crate。
- Ruling: CLI connect 无 GUI 对应期间不进 cli-parity.tsv；TD 落 tunnel_open 后由
  TD 卡补 connect exempt 行（理由引 §19.8）— 依据：TSV 是 GUI↔CLI 映射表。

## 全局约束（逐字进每张任务书）

- 单文件 ≤300 行、单函数 ≤60 行；禁 emoji；失败路径留可观测信号禁静默。
- worktree 全流程；rebase（禁 merge bubble）；提交 `type(scope): subject` + 机理正文；
  中央登记文件（gui-contract/specs/TSV/ai-guide）改动压独立小提交。
- 门禁基线：cargo fmt + clippy -D warnings + 聚焦测试；触 src-tauri 必
  `cargo clippy --all-targets -D warnings`（src-tauri）；GUI 命令卡必 cli-parity +
  ai-docs-sync；前端卡必 gui-check（lint+build+vitest）。
- 主树门禁跑批期间禁对同一 worktree 改文件；验证与提交拆开跑、只 add 显式路径。
- 102 资产：imeepos@192.168.0.102（ssh 免密），Debian headless；wt3c 注入链与
  wt5 跨机拓扑可复用（.orchestrator/2026-09-11-dsh-tunnel/wt3b/、wt5/）。

## 预算

TA 60 / TB 120 / TC 120 / TD 120 / TE 100，分段检查点回报（不设不可执行单总顶）。

## 待细化区

- GUI serve 面多目标白名单（现单 target）：本波不做，记 backlog。
- tunnel_open 多会话并存（现单会话 stop-旧-start-新）：本波不做，记 backlog。
- 分享链接形态（dsh-tunnel-share://）：用户未要求，不做。
