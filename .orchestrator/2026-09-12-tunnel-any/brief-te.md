# 任务书 TE：tunnel 泛化跨机 e2e 证据（102 无头 ↔ mac）

类型：code-quality（证据链）| 预算：100 次调用，分段检查点（30=拓扑起/60=证据采/90=采完）|
分支：feat/wte-e2e-evidence | worktree：.worktrees/wte-e2e（自建，或纯主树只读+远端操作，见边界）
派发者显式 id：session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8
前置：TC（CLI connect）与 TD（GUI tunnel_open）均已合并 main。

## 目标（一句话）

在真实跨机拓扑（102 无头 Debian 被访 ↔ mac 访侧）上，为「任意 HTTP/WS 服务
经 p2p 分享给另一节点使用」采齐硬证据链：非 DSH 的真实 HTTP 服务与 WS echo
服务经 tunnel serve/share → 访侧 CLI connect 与 GUI tunnel_open 双面可用，
HTTP 200 + WS 双向帧 + 双面审计对账 + 回环证明 + 负例 + 优雅收口全 PASS。

## 背景与可复用资产

- 被访侧命令面：`p2pctl tunnel serve --target ...`（W-T5，102 已验过 DSH 形态）。
- 访侧新面：`p2pctl tunnel connect --peer ... --target ...`（TC 落地）+
  GUI tunnel_open（TD 落地，token="" / openUrl=local_addr）。
- 102 资产：imeepos@192.168.0.102（ssh 免密）、Debian 13 headless、node v24
  + pnpm + cargo、上波 clone 的 p2p 仓与已构建 p2pctl（先 git pull 重构建）。
- 证据链先例：.orchestrator/2026-09-11-dsh-tunnel/wt3c/（EVIDENCE-wt3c.md、
  注入驱动链）、wt3b/、wt5/（102 拓扑、审计对账口径）。
- 已知环境坑：DSH_HOME 类持久化状态复跑前必查（wt3c 教训）；mac 无 timeout
  命令，长命令走后台 job；102 无 X/GUI 面。

## 被分享服务（本波「任意服务」证明的关键：不用 DSH）

- HTTP：102 本机起一个非 DSH 的静态 HTTP 服务（python3 http.server 于临时
  目录，含可断言的探针文件），绑 127.0.0.1 随机/固定端口。
- WS：102 本机起 WS echo 服务（首选 node + `npm i ws` 于 /tmp 临时目录的
  20 行脚本；npm 不可达时改 python3 -c websockets 方案或 BLOCKED 呈报候选），
  绑 127.0.0.1。
- 两个端口都进 serve 白名单：`p2pctl tunnel serve --target 127.0.0.1:<http>
  --target 127.0.0.1:<ws>`，stdout ready JSON 行采集 peerId/listenAddrs。

## 证据链（九项，逐项落档 .orchestrator/2026-09-12-tunnel-any/te/EVIDENCE.md）

1. 拓扑建立：102 serve ready 行（peerId/白名单两目标/maxConcurrent）+
   mac 侧节点发现（mDNS 或 bootstrap），真实传输地址非回环。
2. HTTP 面（CLI 访侧）：mac `p2pctl tunnel connect` ready 行 localAddr →
   curl 经 localAddr 取探针文件 200 且内容逐字一致。
3. HTTP 面（GUI 访侧）：GUI tunnel_open 表单填 target+peer → 打开成功 →
   系统浏览器访问 openUrl 截图（探针内容可见）。TD 页面若有注入驱动链可复用
   wt3c；浏览器直达 openUrl 亦可（表单驱动截图 + 浏览器内容截图）。
4. WS 面：mac 侧经隧道对 echo 服务完成一次双向往返（node/python 客户端脚本
   均可，客户端直连 localAddr），采客户端输出 + 服务端收到行日志双向对证。
5. 双面审计对账：访侧 GUI tunnel_status sessions（或 connect 终态输出）↔
   102 serve 侧八字段审计，会话数/字节数同源核对（uid 对齐）。
6. 回环证明：102 `ss -tlnp` 证 serve 与被分享服务仅监听 127.0.0.1；mac 侧
   LocalProxy 亦仅回环（代码面保证 + ss 输出）。
7. 负例：访侧 target 不在白名单 → target_not_allowed（connect 或 GUI 错误
   态呈现）；坏 peer → 连接失败可读错误。
8. 优雅收口：102 serve SIGTERM → stopped JSON 行（served/rejected 计数与
   审计一致）、在途会话排空；mac connect SIGTERM 同口径。
9. 现场清理清单：102 侧进程全退、/tmp 临时目录清除、mac 侧节点/代理退出；
   清理前后 ps/ss 对比留档。

## 验收标准（可机械核验）

- EVIDENCE.md 九项逐项 PASS/FAIL 表 + 每项的命令原文与退出码/截图路径；
  截图入库 .orchestrator/2026-09-12-tunnel-any/te/。
- 全程零代码改动（若有必须改动才能成立的发现 → BLOCKED 呈报，禁顺手修）。
- 分支上仅 .orchestrator/2026-09-12-tunnel-any/te/** 增量（若有环境脚本亦入
  该目录）。

## 边界（明确不做）

不改任何产品代码与契约文档；不在 102 落持久化配置（全部会话态，收口即消失）；
不动上波 wt3b/wt5 现场残留（/tmp/wt3b-* 只读可参考）；不做 DSH 面回归
（W-T5 已验，本卡只证泛化新面）。

## 已知坑

- bash 多字节 locale 变量名炸弹（W1 教训）：脚本写 `${}` 全括护。
- 102 为非 mac：无 pbcopy/无 open 命令；远端命令用 ssh 单条封装，禁交互式。
- wt3c 复跑须知：先查持久化状态（DSH_HOME 类）再起服务。
- 验证命令禁管道收尾吃退出码；长命令后台 job + 日志文件。

## 汇报与停止

DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT + 九项证据表 + 结构化
复盘（环境坑/给后续波的移交）。完成 push origin 分支（仅证据文件），禁自
合并，终报后冻结。停止条件：npm/ws 服务方案不可行且备选不通、102 环境劣化
（cargo build 失败/磁盘不足）、或发现产品缺陷（定格证据后 BLOCKED，由协调者
立修复卡）。