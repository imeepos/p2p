# TE EVIDENCE：tunnel 泛化跨机 e2e 证据（102 无头 ↔ mac）

- 日期：2026-09-11～12（America/Los_Angeles）；基线：origin/main @ **fb70de4f**（fetch 后核对一致）
- 分支 feat/wte-e2e-evidence ｜ worktree .worktrees/wte-e2e ｜ **零代码改动**（分支仅 .orchestrator/2026-09-12-tunnel-any/te/** 增量）
- 拓扑：被访侧 102 = Debian 13 headless（192.168.0.102，ssh 免密），cargo 1.97.1，p2pctl 由 git bundle(feat/wte-e2e-evidence@fb70de4f, 18MB) clone 至 /tmp/te-p2p 后 `cargo build --release`（2m36s）；访侧 mac = 主树同 commit 构建的 p2pctl（apps/cli release, BUILD_EXIT=0）+ p2p-console debug 二进制（20:41 构建，`strings` 验含 tunnel_open）+ apps/gui/dist 重建（含 TD generic 表单标记 `tunnel-generic`）
- 被分享服务（均为非 DSH 真实服务，102 本机 127.0.0.1）：
  - HTTP：`python3 -m http.server 18081 --bind 127.0.0.1`（/tmp/te-http，探针 `probe.txt` = `te-probe-fb70de4f-1789187193`，29 字节）
  - WS：node v24.18.0 + `npm i ws@8`（NPM_OK），echo 服务 `node echo.js` 绑 127.0.0.1:18082（20 行脚本，连接/收帧/关闭全 JSON 行日志）
- serve 白名单：`p2pctl tunnel serve --target 127.0.0.1:18081 --target 127.0.0.1:18082 --data-dir /tmp/te-serve-node`

## 九项证据表

| # | 验收项 | 判定 | 命令原文与关键输出（退出码） | 证据文件 |
|---|---|---|---|---|
| 1 | 拓扑建立 | **PASS** | serve stdout ready 行：`{"allow":["127.0.0.1:18081","127.0.0.1:18082"],"kind":"ready","listenAddrs":["127.0.0.1/u60961","127.0.0.1/t40957"],"maxConcurrent":16,"peerId":"HTrs9PNhvkstv4aP6scfH9r9DQHdfPYjPrpEstFUNrjU"}`；mac 侧无 bootstrap 参数下 mDNS 自动发现，CLI connect 约 2s 出 ready 行；GUI 节点（peer `7DhU9rUD46bELUcF82rpWiTCWwfns1jDxWEtCYBEBGAb`）与两个 CLI connect 节点（`CTQZ2ogn…`/`GsbQpwgS…`）均跨机到达 102（102 审计 peer_id 在案）。跨机直连排除回环可能：`curl http://192.168.0.102:18081/probe.txt` exit=7、mac 本机 `curl 127.0.0.1:18081` exit=7（连接拒绝，服务未暴露非回环面）。传输层地址 grep 见 raw/transport-grep.txt | raw/serve-102.out |
| 2 | HTTP 面（CLI 访侧） | **PASS** | `p2pctl tunnel connect --peer HTrs9…FUNrjU --target 127.0.0.1:18081 --data-dir /tmp/te-visit-node-cli` → `{"kind":"ready","localAddr":"127.0.0.1:52927","peerId":"HTrs9…","target":"127.0.0.1:18081"}`；`curl -o … -w "%{http_code}" http://127.0.0.1:52927/probe.txt` → **http_code=200 exit=0**；`cmp` 访侧取回文件 vs 102 `cat probe.txt` → **CONTENT_IDENTICAL**（逐字一致） | raw/connect-http.out |
| 3 | HTTP 面（GUI 访侧） | **PASS** | 注入驱动链（te/static-injector.mjs 双栈 5173 + te/driver.js）驱动 debug p2p-console 真实 webview：`fill_generic{port:18081, peer:HTrs9…}` → 回执 `clicked:"打开通用服务"`；状态卡（driver text 命令 c2）：`已开启 / 本地地址 127.0.0.1:53436 / 浏览器入口 http://127.0.0.1:53436 / 隧道目标 127.0.0.1:18081`；`tunnel_status`（c3）：`{active:true, localAddr:"127.0.0.1:53436", target:"127.0.0.1:18081"}`；控制通道截图（Authorization: Bearer token）：`POST /screenshot` → PNG 1080×800（表单成功态）；系统浏览器（playwright 驱动）直达 openUrl `http://127.0.0.1:53436/probe.txt` → a11y 文本 = `te-probe-fb70de4f-1789187193`（探针逐字可见）+ 截图 | gui-form-opened.png, openurl-browser-probe.png, raw/driver-results.json |
| 4 | WS 面 | **PASS** | `p2pctl tunnel connect … --target 127.0.0.1:18082` → ready localAddr `127.0.0.1:53010`；`node ws-client.mjs ws://127.0.0.1:53010` → `open → text-echo te-echo-text-1789187506705 → binary-echo 010203fafbfc → close 1000`，WS_CLIENT_EXIT=0；102 echo 服务端日志同一毫秒窗：`connection remote=127.0.0.1 / recv te-echo-text-… / recv <bin:6> / close`（双向帧对证） | raw/connect-ws.out, raw/102-services.log, te/ws-client.mjs |
| 5 | 双面审计对账 | **PASS** | GUI `tunnel_status.sessions`（driver invoke c4）2 条：`85f786ec7cc00076`（bytesIn=29, ok）与 `aa7c97ad6a8e299a`（bytesIn=335, ok），peerId/target/时间戳齐；102 serve 审计同 id 两条：`85f786ec… bytes_in=907 bytes_out=215 served`、`aa7c97ad… bytes_in=834 bytes_out=520 served`——**session_id 逐字对齐**。CLI 两面：connect stopped 行 served=1/sessions=1 ×2 ↔ 102 审计 `bce13ac32da36c7a`/`e91d3b7c62a362df` 各 1 条 served。字节方向语义注记：访侧计 post-head 泵字节（bytesIn=响应 body 29/335），被访侧计原始连接字节（含 head，907/834 与 215/520）——同源不同口径，uid 对齐为对账锚 | raw/driver-results.json, raw/serve-102.out |
| 6 | 回环证明 | **PASS** | 102 `ss -tlnp`：`127.0.0.1:18082 (node echo)`、`127.0.0.1:18081 (python3)` 仅回环；p2pctl 自身仅节点传输口 `0.0.0.0:40957(tcp)`/quic udp（p2p wire 面设计如此，非代理面）；102 上无任何被分享服务的非回环监听。mac 直连负探针双向 exit=7（上表#1）。mac 侧 LocalProxy：三 connect ready 行 localAddr 全部 `127.0.0.1:xxxxx`（52927/53010/54835），GUI openUrl=`127.0.0.1:53436`——代码面（local_proxy 绑定回环）+ 输出面双证 | raw/serve-102.out, raw/connect-*.out |
| 7 | 负例 | **PASS** | ①白名单外：`connect --target 127.0.0.1:9999` → localAddr 54835 ready 后 `curl http://127.0.0.1:54835/probe.txt` → **code=502**、body=`p2p tunnel proxy error 502: 隧道开启失败: tunnel rejected: target_not_allowed (gate: target_not_allowed)`（短应答不悬挂，curl_exit=0）；102 审计 `outcome=rejected:target_not_allowed bytes_in=0 bytes_out=0`。②坏 peer：`connect --peer 1111…1(43字符)` → `p2pctl: 运行失败: --peer 长度非法（须 32 字节）`（可读校验错误，即刻失败） | raw/connect-neg.out, raw/connect-badpeer.out, raw/neg-body.txt |
| 8 | 优雅收口 | **PASS** | mac 访侧 ×3（长跑 connect SIGTERM）：stopped 行 `{"broken":0,"kind":"stopped","rejected":0,"served":1,"sessions":1}`（http/ws 两面）+ neg 面 `{"broken":1,…,"sessions":1}`（broken=1 即 502 短应答连接）；退出码锚点（专用 wrapper 轮）：`pkill -TERM -x p2pctl` → **EXIT=0** + stopped 行。102 被访侧：`kill -TERM <serve_pid>` → `{"broken":0,"kind":"stopped","rejected":1,"served":4,"sessions":5}`（served=4 与 4 条 served 审计一致、rejected=1 与 rejected 审计一致）；进程退出（raw/102-after-cleanup.txt p2pctl_procs=0） | raw/connect-exit.out, raw/102-after-cleanup.txt |
| 9 | 现场清理 | **PASS** | 102：pkill http.server/echo.js + `rm -rf /tmp/te-http /tmp/te-ws /tmp/te-p2p /tmp/te-serve-node /tmp/te-serve.out …`；after：listeners=0、p2pctl_procs=0、/tmp/te-* 残留=0（svc_procs 计数 2 系远端 shell cmdline 自匹配假象，复核无匹配进程、端口清零，见 raw 注记）。mac：injector(5173)/GUI(31790)/全部 connect 进程退出 + /tmp/te-* 全清（after ps 存 raw/mac-after-cleanup.txt）。清理前监听态见各 raw 文件 | raw/102-after-cleanup.txt, raw/mac-after-cleanup.txt |

## 过程资产（te/ 目录）

- static-injector.mjs：wt5 改版——绑双栈（wt3c 教训：devUrl=localhost 解析 ::1）、请求日志 /tmp/te-injector-reqs.log
- driver.js：wt3b 协议兼容，新增 `fill_generic`（TD 表单 `#tunnel-generic-target`/`#tunnel-generic-peer` + 卡内按钮）；boot 回执含 dataDir/href（raw/driver-results.json）
- ws-client.mjs：node24 内置 WebSocket，文本+二进制双帧 echo
- raw/：serve-102.out（ready+全部审计+stopped）、connect-{http,ws,neg,badpeer,exit}.out、driver-results.json、102-services.log、102-after-cleanup.txt、mac-after-cleanup.txt、neg-body.txt、transport-grep.txt
- 截图：gui-form-opened.png（GUI 表单成功态，控制通道 /screenshot）、openurl-browser-probe.png（系统浏览器经 openUrl 的探针内容）

## 偏差与观察（只记录，未动代码）

1. **GUI 窗口截图陈旧帧**：窗口被遮挡时控制通道 /screenshot 返回上一次合成帧（两次截图字节完全相同）；`osascript` 置 frontmost 后 FRAME_FRESH。采证脚本化时需置前再拍。
2. **字节口径非对称**：访侧 session bytesIn 只计 body 泵字节（head 单列），被访侧 bytes_in/out 计原始连接字节——对账以 session_id/会话数/方向流量趋势为锚（§19 八字段形状一致）。
3. **rejected 审计行打印两次**（同 session_id 两行，04:36:40.217100 同时间戳）：观察项，疑似 reject 路径双记，不阻断对账（rejected=1 计数正确）。
4. **control 通道 ROUTES 白名单无 remote-access**：/navigate 与 /page/action 到不了该页（404/400），故表单驱动走 wt3b 注入链（本卡 driver.js 适配 TD 表单后可用）；/invoke(只读子集外)+/screenshot 全局可用，截图链因此可行。
5. **ready 行 listenAddrs 显示 127.0.0.1**：102 serve ready 行 listenAddrs 为回环字面量（无 configured advertised addrs 时），跨机互连实际走 mDNS 发现的 LAN 地址（访侧无 bootstrap 连通为证）；显式传输地址对证留 raw/transport-grep.txt。
6. 上波残留/本波环境：wt5-dsh systemd 单元未触碰；102 /tmp 上一波 wt5-* 残留只读未动（按边界）。

## 环境坑与移交（结构化复盘）

- **pkill -f 自匹配**：pkill/ssh 远端命令行含模式串时会自杀（本波两次踩中：exit-code wrapper 被 -f 误杀、102 清理 ssh 会话被自匹配中断）；一律 `pkill -x <comm>` 或 `pkill -f "[x]yz"` 括号 trick。
- **GUI devUrl 双栈**：debug 二进制 devUrl=localhost:5173，注入服务器必须绑双栈（node listen 省略 host），仅 127.0.0.1 会因 ::1 解析失败。
- **dist 新鲜度**：apps/gui/dist 是构建产物，主树 dist 可能落后于刚合并的前端——GUI 证据前必须验证 dist 含目标 DOM 标记（grep `tunnel-generic`）。
- **102 仓分发**：~/src/p2p 为无 .git 旧拷贝（9/2），跨机传仓用 `git bundle create <branch>` + scp + `git clone -b <branch> bundle`（18MB，clone 即含分支）；apps/cli 独立 workspace，`cd apps/cli && cargo build --release` 2m36s（102）。
- **npm i ws@8** 于 102 可达 registry（5s 装完）；node24 内置 WebSocket 客户端可直接写 echo 客户端，零依赖。
- **控制通道鉴权**：`Authorization: Bearer $(cat <dataDir>/control/token)`，端点在 `<dataDir>/control/endpoint.json`；P2P_CONTROL_PORT 环境变量指定端口，被占即报错不静默换口。
