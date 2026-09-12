# W-T5：headless serve 面（p2pctl tunnel serve）+ busy 栅栏对账 + 102 跨机实证

## 背景与目标（一句话）
被访侧 responder+serve 开关目前只活在 GUI 进程（TunnelState），无头机起不了被访侧，
跨机验收断链——本卡按 gui-contract §19.8 预留条款落地 `p2pctl tunnel serve` headless
面，复用 crates/p2p-tunnel responder，随后在 102（无头 Debian）跑真跨机全链实证。
用户已批（2026-09-12 方案甲）。

## 事实（已核查，直接用）
- 102：`ssh imeepos@192.168.0.102`（免密），Debian 13 无头；node v24.18.0 在
  `/home/imeepos/.nvm/versions/node/v24.18.0/bin`（非交互 ssh 需显式带 PATH）、pnpm、
  cargo /usr/bin/cargo；无 p2p 仓库、无 DSH（DSH 是 npm 包可全局装，参照
  102:~/dsh-lan-setup.md 的部署模式）。mac 本机为访侧（GUI + wt3c 注入驱动链可复用，
  见 .orchestrator/2026-09-11-dsh-tunnel/wt3b/、wt3c/）。
- serve 契约：规范页 §5.2（enabled 会话态默认关、白名单 127.0.0.1 精确匹配、终态
  可观测）；GUI 侧同语义实现在 apps/gui/src-tauri/src/tunnel/（只读参照，禁改）。
- busy 栅栏现状：crates/p2p-tunnel serve 并发许可默认 4；浏览器 burst（峰值 6-14
  并发）必现 busy→502，量化数据在 .orchestator/2026-09-11-dsh-tunnel/wt3c/
  EVIDENCE-wt3c.md「busy 栅栏缺口复现参数」节。

## 任务分段（每段末回报，预算分段）
1. **CLI 子命令**：`p2pctl tunnel serve --target 127.0.0.1:<port> [--allow ...]
   [--max-concurrent N]` 前台常驻进程（进程活=enabled，SIGINT/SIGTERM 优雅收口且
   每条活动会话落终态，禁静默吞错）；节点装配复用既有 CLI 装配模式（先 recon
   `p2pctl acp console` 的前台装配先例与 node bootstrap 面，不新造轮子）；结构化
   日志输出会话审计八字段。契约：specs/tunnel.md §5.2 扩 headless 面条款 +
   gui-contract §19.8 措辞落地（仍是独立进程面，GUI serve_start 语义不混同）。
2. **busy 栅栏对账**：默认值裁决（建议上调并给 `--max-concurrent` 覆盖口）+ 红绿
   itest（并发 burst 场景：低于许可全过/超许可部分 busy 且逐条落终态）+ 规范页
   §5.2 同步。GUI 侧默认值联动：GUI 不改代码，读同一常量即自然受益，验证引用关系。
3. **登记面**：ai-docs-sync（新命令条目+参数表+示例，实测可跑）；cli-parity.tsv
   tunnel_serve_start/stop 两行豁免理由更新（补「headless 对等面=p2pctl tunnel
   serve 独立进程」一句，仍 exempt：GUI 命令开关的是 GUI 常驻节点，非同一对象）；
   protocol-registry 不动（无新协议）。
4. **102 跨机实证**：102 上 clone 本仓（origin=GitHub imeepos/p2p）+ cargo build
   p2pctl + npm 全局装 DSH 并起 web（DSH_HOME=/tmp/wt5-dsh-home，仅 loopback）→
   `p2pctl tunnel serve --target 127.0.0.1:<dsh_port>` → mac 访侧 GUI 开隧道（复用
   wt3c 驱动链）→ 证据链：两节点跨机互连（peerId 各异、非回环传输地址）、WS 101、
   真实流式对话、审计与日志双面留痕、lsof 102 侧仅 loopback 监听、坏 token 401 负例。
   证据落 .orchestrator/2026-09-11-dsh-tunnel/wt5/。
5. **门禁**：触及 crate clippy --all-targets -D warnings + 全量 make check
   （CI=true LANG/LC_ALL=en_US.UTF-8）EXIT=0 → push origin feat/wt5-headless-serve，
   禁自合并。

## 边界（明确不做）
- 禁改 apps/gui/src-tauri/**（GUI 零改动；其默认值联动仅验证引用）。
- 不做 daemonize/开机自启（前台进程足够验收；114 机为可选延伸非验收项）。
- 不动远端协议（无新协议 ID）；无迁移预期，若涉迁移先查两处占号。

## 验收标准（可判定）
1. 红：busy 栅栏 burst itest 在修复/裁决前红（留 RED 记录）。
2. 绿：tunnel serve 单测+itest 全绿（含优雅收口终态断言）；llm-share 与 tunnel_wave
   既有套件零回归。
3. ai-docs-sync 末行 AI-DOCS-OK（含新命令）；cli-parity 末行 CLI-PARITY-OK。
4. 102 全链：跨机 peerId 证据 + WS 101 + 流式对话截图 + 双面审计 + 401 负例 +
   loopback 监听证明，全部落 wt5/ 目录带时间戳。
5. 全量 make check EXIT=0（你的 worktree）。

## 预算与停止
- 总顶 200 次调用，五段分段检查点回报，段末超支预警（e2e 段④单独 60 上限）。
- 102 环境不可恢复阻塞（如 GitHub 不可达、apt 无 sudo）→ 立即 BLOCKED 回报，
  带已试命令与退出码，不要绕路硬试。
- 产物纪律：写完即 commit；分支 feat/wt5-headless-serve；派发者显式 id
  session-3373f897-f9c0-4a32-9af2-b9562eecc311（send_parent 异常时定向投递）。
