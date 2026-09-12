# W-T3b 证据归档（2026-09-11）

## 结论：DONE_WITH_CONCERNS —— 证据链 1/2/5(半)/6/7/8 PASS，3 半 PASS，4 FAIL（硬项缺，根因已定位）

## 拓扑（全链真实组件）
- 实例 A：pid 见日志，`P2P_CONTROL_PORT=31780`，节点 peer `5VBVBUpkzqKCtJ3koFabx1wWSBmuEWk9FjSqq3SVWZkY`，dataDir `/tmp/wt3b-node-a`
- 实例 B：`P2P_CONTROL_PORT=31781`，节点 peer `2VMiV9DWCXPKXmEUXBqhh3DE7zRYAYYjdJzjNg6xB7iU`，dataDir `/tmp/wt3b-node-b`
- 被访 DSH：`DSH_HOME=/tmp/wt3b-dsh-home dsh web --no-open`（vite-plus 0.1.5-rc.2 产品 CLI；ymm-001 checkout 的 .env 自带 DSH_HOME 被其启动守卫拒绝，故用同版本产品 CLI，见"局限"）
  boot URL：`http://127.0.0.1:3080/?token=IXXs8L9aKgOcsCEZUl-NB6Ku1ZRCLnQLKcPeV-myiLw`
- 驱动：`.orchestrator/2026-09-11-dsh-tunnel/wt3b/driver-proxy.mjs`(5173→vite5174) + `driver.js`（仓库零改动；机制=webview 内经真实 `__TAURI_INTERNALS__.invoke` 调 Tauri 命令 + DOM 驱动真实表单按钮）

## 证据链逐项
1. **A 侧 serve 开启 PASS**：`tunnel_serve_start("127.0.0.1:3080")` 返回 `{enabled:true, allow:["127.0.0.1:3080"], activeSessions:0}`；`tunnel_status` serve 字段同值（§19 形状）。
2. **B 侧开隧道 PASS**：远程访问视图真实表单（`#tunnel-dsh-url`/`#tunnel-peer` 填入 + 点击真实按钮「开启远程访问」）→ `tunnel_status` = `{active:true, localAddr:"127.0.0.1:52172", target:"127.0.0.1:3080", sessions:[…peerId=5VBV…(A)]}`。
3. **浏览器首屏 半 PASS**：`open_url=http://127.0.0.1:52172/?token=…` 经隧道返回 DSH HTML（title=DeepSeek Harness，插件清单齐全，4.3MB vendor 资产经隧道字节面流过）；但 /api 层 403（见 4）导致应用降级，非空白（`wt3b-gui-b-open.png` 为 GUI 面；浏览器面截图副本未归档成功，浏览器 console/网络日志为准）。
4. **remote.mux WS + 流式对话 FAIL（硬项缺）**：`ws://127.0.0.1:52172/api/remote.mux` 握手 403；所有 `POST /api/*` 403，响应体即 DSH 原文 `forbidden`。**判别实验**：同浏览器直连 `127.0.0.1:3080` 全部 `/api/*` 200（无 403）。GET 经隧道 200、POST 经隧道 403 → 定位：B 反代只重写 Host，未改写 Origin/Referer（浏览器 Origin=http://127.0.0.1:52172），DSH 网关按 authority 校验拒绝写请求与 WS。**缺陷记录（未改码）**：apps/gui/src-tauri/src/tunnel/head.rs（Host 重写处）不覆盖 Origin；复现=本链步骤 2+3。
5. **Host 重写 半 PASS**：DSH 对经隧道请求正常响应（含 4.3MB 资产与 401/403 语义响应）说明 Host=127.0.0.1:3080 被接受；但 Origin 未重写引出 4 的 403，authoritative 实测留待 DSH 侧日志（本轮未取到 DSH 访问日志行）。
6. **回环证明 PASS（反代）**：`lsof -p <B>` LISTEN = `localhost:31781`(控制通道)、`localhost:52172`(反代)、localhost:52057/52058；无 `*:52172`。观察点（非隧道域）：p2p 节点自身 swarm TCP 监听为 `*:52056`/`*:52080`（节点网络面，固有其用，记录备查）。
7. **负例 PASS**：坏 token 直连 DSH → 401；坏 token 走隧道 → 401（隧道透传 DSH 401）。GUI 错误三态：`http://0.0.0.0:1/?token=bad` → 视图「错误」badge + 「最近错误: host 只允许 127.0.0.1 回环字面量: 0.0.0.0」+ 节点提示语（driver DOM 文本全文在案）。
8. **审计行 PASS**：`tunnel_status.sessions` 80+ 条八字段记录（sessionId/peerId/target/startedAt/endedAt/bytesIn/bytesOut/outcome），真实流量字节（如 bytesIn=4332962 的 vendor chunk、18478/191898 等），终态闭集 `ok/busy/io/open` 均实测出现。
9. **清理**：见 PROGRESS 收尾（两实例/DSH/vite/代理全部退出；gui-config.json 恢复备份；/tmp/wt3b-dsh-home 留作复跑）。

## 发现缺口（只记录不改）
- remote-access 路由不在控制通道 ROUTES 白名单（control/mod.rs 11 条），page-registry 无 remote-access 条目 → tunnel 四命令无任何现成外部驱动面；视图亦无 serve 开关表单（tunnel_serve_start 无 GUI 入口）。
- macOS AX(TCC) 拒绝 → 物理点击路径不可用（本机约束，非代码缺陷）。
- 控制通道 /screenshot 帧源滞后：三次捕获字节数全等（41758），视图已变仍返回同一帧（control/capture.rs）；DOM 文本经 driver 取证可靠。
- GUI 错误态与「已开启」卡并存展示（错误不回退快照的副作用），信息架构可议。
- gui-contract §19 的 peer 参数已实测必需（visit.rs 校验"未指定被访节点 peer"报错）。

## 局限
- DSH 用 vite-plus 产品 CLI（0.1.5-rc.2）而非 ymm-001 checkout 裸跑：checkout `.env` 自带 DSH_HOME 触发其启动守卫直接拒绝（app-boot/src/index.ts:175），不改他仓文件为红线，故以同版本产品 CLI 等价起服。
- 硬项 #4 缺：根因明确（Origin 未重写 → DSH 403），修复属代码改动（W-T3/W-T4 域），本轮仅记录。
