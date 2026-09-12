# W-T3b：DSH 隧道访侧 GUI 全链 e2e 实证补做（证据卡，预算封顶）

## 背景与目标
W-T3 代码已合入 main（tunnel 访侧 + W-T2 被访侧，全量门禁绿）。本卡**只做实证、原则上零代码改动**：
补齐 gui-contract §19 验收要求的双实例 GUI 全链 e2e 证据链。工作目录：本仓库自建 worktree
（如需临时脚本放 `.orchestrator/2026-09-11-dsh-tunnel/wt3b/`，不进 crates/apps）。

## 拓扑与实例隔离（先确认再动手，防烧预算）
- 被访侧 = p2p GUI 实例 A（节点起、tunnel responder 随 W-T2 装配、`tunnel_serve_start("127.0.0.1:<dsh_port>")`）。
- 被访服务 = 真 DSH web：`DSH_HOME=/tmp/wt3b-dsh-home dsh web --no-open`（隔离实例，
  W-T3 已验证可起、boot URL 可捕获、错误 token 401 有明确提示）。
- 访侧 = p2p GUI 实例 B（与 A 不同数据目录/端口；动手前先确认 GUI 的实例隔离机制：
  数据目录与监听端口如何随 env/配置分开，`apps/gui` 启动装配与 `.env` hook。确认不了就先报阻塞，不要盲试）。
- A、B 两节点互连（好友/拨号用仓库既有 CLI 或 GUI 面均可，以能建立流为准）。

## 证据链清单（每项都要留痕，全链一次跑通后按序截图/取证）
1. A 侧 serve 开启：`tunnel_serve_start` 后 tunnel_status emit（serve.enabled=true、allow 含目标）。
2. B 侧开隧道：远程访问视图粘贴 DSH 启动 URL → `tunnel_open_dsh` → 返回 open_url。
3. 浏览器打开 open_url：DSH 首屏渲染成功（截图）。
4. `/api/remote.mux` WS 握手成功 + 一次真实对话流式消息走通（截图/录屏或网络面板证据）。
5. Host 重写观测：B 侧反代发出的请求 Host=`127.0.0.1:<dsh_port>`（proxy 日志/审计行证据；
   结合 DSH 303 set-cookie authority=127.0.0.1:<port> 实测，W-T3 已证 half）。
6. 回环证明：`lsof` 显示 B 侧反代监听仅 127.0.0.1（无 0.0.0.0/::）。
7. 负例：过期/错误 token 的启动 URL → DSH 401（W-T3 已实测，可复用）+ 隧道 error 帧路径
   的 GUI 错误三态展示（截图）。
8. 审计行：B 侧远程访问视图出现会话审计行（八字段，终态落 outcome）。
9. 收尾清理：杀两实例与 DSH 实例，`/tmp/wt3b-dsh-home` 可留作复跑；不留 acc_ 造数。

## 红线
- 禁改 crates/apps 代码；发现代码缺陷只记录（文件:行 + 现象 + 复现步骤），不开改。
- 禁跑 `make check`（证据卡无代码变更，门禁由协调者侧覆盖）。
- 每阶段出阶段性证据即可回报，不憋终稿；预算封顶 120 次工具调用，超限前必须回报状态。
- 实例/端口/目录命名全部带 wt3b 前缀，严禁触碰其他会话 worktree 与运行中实例。

## 验收裁定口径
证据链 1-8 齐 = 全链 PASS；3/4（真实 DSH 流式对话）为硬项，缺任一 = DONE_WITH_CONCERNS
如实写缺什么、卡在哪。协调者按红绿双向标准复核证据后补记 W-T3+W-T3b 合并验收裁定。
