# 任务书 TD：GUI 通用入口 tunnel_open + remote-access 页泛化

类型：frontend | 预算：120 次调用，分段检查点回报（40/80/120）|
分支：feat/wtd-gui-open | worktree：.worktrees/wtd-gui（自建，基于最新 origin/main）
派发者显式 id：session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8
前置：TB 已合并（crate 面与 GUI 消费面就位）。

## 目标（一句话）

把「打开远端任意 HTTP/WS 服务」做成 GUI 一等入口：新增 tunnel_open(target,
peer) 命令（gui-contract §19.3 约束 9 已冻结形状），remote-access 页在 DSH
专用入口之外提供任意 target+peer 的通用打开表单。

## 背景与真值源

- 契约：docs/design/gui-contract.md §19.1 命令表 tunnel_open 行 + §19.3 约束 9
  （target 字面量服务端校验 / 复用 TunnelOpenReport / token="" 语义禁拼
  ?token= / openUrl=local_addr / 事件复用 tunnel_status / tunnel_open_dsh
  保留并存）。
- 现状代码：apps/gui/src-tauri/src/tunnel/visit.rs（tunnel_open_dsh 与
  TunnelState::start 已参数化，照抄其装配路径）；types.rs（From 映射已就位）；
  tb-PROGRESS.md 的 TD 移交注记。
- 前端：apps/gui/src（remote-access 页——先读 src/ 下该页面与 i18n 登记面）。

先读：AGENTS.md → 本任务书 → gui-contract §19.3 约束 9 → visit.rs/types.rs →
remote-access 页现状。

## 产出物

1. src-tauri 侧：visit.rs 增 tunnel_open(app, state, target, peer) 命令函数
   （target=127.0.0.1:<port> 字面量解析校验、peer 同既有 parse_peer、装配
   NodeTunnelOpener 复用、TunnelState::start 复用、open_url=local_addr、
   token=""）；tunnel.rs 薄包装 #[tauri::command]（**禁三段路径**——tauri
   命令宏不跟随 use re-export，KI:601）；lib.rs 命令表在 tunnel_open_dsh 行后
   插入 tunnel_open 行（既有四行序不变）。
2. cli-parity.tsv：+1 行 tunnel_open 映射或 exempt（tunnel_open_dsh 同款判定：
   GUI 命令有 CLI 对等——TC 已落地 p2pctl tunnel connect，可映射则映射，语义
   不对等则 exempt 并引 §19.3 约束 8/9 理由；读 tsv 现行格式自取行样式）。
3. 前端（apps/gui/src）：remote-access 页增「通用服务」表单（target 端口输入
   + peerId 输入 + 打开按钮），复用既有错误展示与 tunnel_status 事件订阅；
   i18n 三处登记（types+locale）随独立小提交；空态/错误态/加载态三态齐全；
   成功态展示 local_addr 可复制链接。**不改 DSH 专用入口既有交互**。
4. .orchestrator/2026-09-12-tunnel-any/td-PROGRESS.md。

## 验收标准（可机械核验，逐条带退出码）

- src-tauri：cargo test -p p2p-console EXIT=0（含 tunnel 面既有 7+ 用例）；
  cargo clippy -p p2p-console --all-targets -- -D warnings EXIT=0（在
  apps/gui/src-tauri 目录内跑——双 workspace 结构，根 workspace exclude）。
- 前端：apps/gui 目录 pnpm lint EXIT=0；pnpm test EXIT=0（新增表单至少 1 条
  组件测试覆盖成功/错误两态）；pnpm build EXIT=0。
- 契约对账：cli-parity 门禁 EXIT=0（含新行）；ai-docs-sync EXIT=0（若 tunnel
  命令面文档需同步）。
- bash scripts/check/tests/line-limit.sh EXIT=0（新文件行数机械兜底；2026-09-12
  教训：make 级门禁入自验清单，禁只跑单点命令）。
- git diff main --name-only 全部落在 apps/gui/（src-tauri+src）+ cli-parity.tsv
  所在路径 + i18n 登记文件 + .orchestrator/2026-09-12-tunnel-any/。
- 界面调用点对账（红线）：tunnel_open 前端必须有真实调用点（表单提交），
  不许「契约三层就绪零调用点」。

## 边界（明确不做）

不改 crates/**、apps/cli/**、specs/gui-contract 契约文本（§19.3-9 已冻结，
发现形状问题 BLOCKED 回报）；不做多会话并存（单会话 stop-旧-start-新 语义
不变）；不动 tunnel_open_dsh 既有交互与文案。

## 已知坑

- zustand selector 禁每次新建数组/对象当快照（引用漂移无限重渲白屏，2026-09-03
  实证）；路由冒烟禁只踩默认路由；spawn/异步求值同 TB 任务书坑位。
- 前端错误禁只进 console——复用页面既有落盘/桥接通道。
- 验证与提交拆开跑；commit -F -；只 add 显式路径。

## 汇报与停止

DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT + 行为证据（命令+真实
退出码+截图证据链：表单→打开→成功态 local_addr 展示→错误态（坏 target））+
结构化复盘。完成 push origin 分支，禁自合并，终报后冻结。停止条件：契约形状
与现实现冲突、或 remote-access 页结构使表单无法不破坏 DSH 入口而加入。
