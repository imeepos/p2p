# W-T3c 段③全链复跑证据（2026-09-11，硬项 #4 FAIL→PASS）

## 拓扑（复用 wt3b 注入驱动链，代码侧为 wt3c 修复版）
- GUI A（被访侧）：`P2P_CONTROL_PORT=31780 p2p-console`（worktree ../p2p-wt3c debug 构建，
  含 3635fb47 Origin/Referer 重写修复），dataDir /tmp/wt3b-node-a，peer `5VBV…SVWZkY`
- GUI B（访侧）：`P2P_CONTROL_PORT=31781`，dataDir /tmp/wt3b-node-b，peer `2VMi…6xB7iU`
- DSH：`DSH_HOME=/tmp/wt3c-dsh-home dsh web --no-open` → `http://127.0.0.1:3080/?token=i_x0…`
  （**换新 home**：复跑发现 /tmp/wt3b-dsh-home 持久化了 wt3b 轮注册的反代地址 52172，
  页面 mux/gateway 指向已死端口，必须隔离重起）
- 驱动：wt3b/driver-proxy.mjs（5173→vite5174，本次运行绑双栈）+ driver.js（零改动）

## 硬项 #4 逐条翻转
1. **B 侧开隧道 PASS**：真实表单 fill_open → `tunnel_status = {active:true,
   localAddr:"127.0.0.1:64050", target:"127.0.0.1:3080"}`（重开后新端口 64050）。
2. **浏览器经隧道加载 PASS**：`GET /?token=… → 303 → 200`；插件 bundle/index/manifest 200；
   4.3MB vendor chunk 多次经隧道通过（bytesIn=4332962 实测 2 次）。
3. **/api 全 200 PASS（wt3b 为全 403）**：`POST /api/settings/describe`、
   `/api/session/list`、`/api/credentials/describe`、`/api/agentPresets/list`、
   `/api/session/modelCatalog` 等经隧道全部 200（网络日志在案）。判别对照：wt3b 同路径全 403。
4. **remote.mux WS 握手 PASS（wt3b 为 403）**：页面上下文发起
   `new WebSocket('ws://127.0.0.1:64050/api/remote.mux')` → **onopen（HTTP 101 升级成功），
   3ms**。浏览器自动带 `Origin: http://127.0.0.1:64050`，反代重写后 DSH authority 栅栏放行。
5. **真实流式对话 PASS（wt3b 未达成）**：经隧道 UI 实操——API 建 workspace（wt3c-demo-ws）、
   Settings→Models 加 custom provider（moonshot/https://api.moonshot.cn/v1，kimi-k3）、
   选模型、发送「请用一句话回答：1+1等于几？」→ 流式回包 **「1+1等于2。」**
   （Usage 7.8K tok，Ran for 4s，58 tok/s，1 turns 1 steps；截图 wt3c-full-chain-chat.png，
   DOM 文本同证）。
6. **审计行 PASS**：`tunnel_status.sessions` 225 条（ok 156 / io 41 / busy 26 / open 2），
   累计 bytesIn=12,229,741 / bytesOut=41,164，八字段形状同 §19。

## 过程发现（只记录不改动，供协调者裁决）
- **busy 栅栏并发上限 4（crates/p2p-tunnel config 默认 max_concurrent:4）**：浏览器并行
  asset/API burst 下 502（"tunnel rejected: busy"），页面需刷新 1-2 次才能拿全资源。
  wt3b 轮已潜伏，本轮实测放大。修法属 W-T4 域（crates/**）或代理侧重试，未动手。
- 驱动链基建备忘：webview 后台节流使 driver 轮询迟滞（分钟级），步进需长等待；
  driver-proxy 本轮运行副本绑双栈（wt3b 原版仅 127.0.0.1，localhost 解析 ::1 时失败）。
