# TG1 · tunnel「分享任意 http/ws 给另一节点」GUI 差距矩阵与交互设计

日期：2026-09-14 · 类型：只读调研（零生产代码改动） · 结论：**DONE**
判定基线：IPC/契约面与任务书描述一致，`tunnel_open` 未废弃（§19.3-9 在册且已实现），无 NEEDS_CONTEXT 事由。

## 1. 能力面全景表

### 1.1 CLI 参数面 → GUI 操作面

| CLI 参数 | 出处 | 语义一句话 | GUI 操作面 | 判定 |
|---|---|---|---|---|
| `serve --target ×N` | apps/cli/src/tunnel/serve.rs:25-26 | 白名单目标（精确 `127.0.0.1:<port>`，可多个） | TunnelServeCard 单端口输入→`tunnel_serve_start`（tunnel-serve-card.tsx:35-50），白名单逗号串展示（tunnel-serve-card.tsx:135-139） | 有（每次一端口） |
| `serve --allow ×N` | serve.rs:28-29 | 同 target 的追加语义 | 同上，GUI 无独立入口（等价累积） | 有（等价） |
| `serve --max-concurrent` | serve.rs:31-32 | 并发许可覆盖（默认 16，config.rs:32-37） | 无任何命令/字段暴露 | **缺** |
| `serve --data-dir/--quic-port/--tcp-port/--no-mdns/--bootstrap` | serve.rs:34-47 | headless 节点装配参数 | 不适用（GUI 节点由主进程装配） | 不适用 |
| `connect --peer` | apps/cli/src/tunnel/connect.rs:27-28 | 被访节点 PeerId（base58） | 手输 Input（generic-tunnel-card.tsx:82-92） | 有（无好友选择器） |
| `connect --target` | connect.rs:30-31 | 目标字面量（须在对端白名单） | 端口输入自动拼 `127.0.0.1:`（generic-tunnel-card.tsx:61-77） | 有 |
| `connect` 节点装配参数 | connect.rs:33-46 | 同 serve | 不适用 | 不适用 |

### 1.2 IPC 命令面 → GUI 操作面

命令注册：apps/gui/src-tauri/src/lib.rs:125-129；前端类型：apps/gui/src/lib/ipc-types.ts:744-795。

| IPC 命令 | 契约 | 语义一句话 | GUI 操作面 | 判定 |
|---|---|---|---|---|
| `tunnel_open_dsh(url, peer?)` | gui-contract.md:760 | DSH 启动 URL→隧道→反代→开浏览器 | DSH 卡（remote-access-view.tsx:134-158，remote-access-view.tsx:71-92） | 有 |
| `tunnel_open(target, peer)` | gui-contract.md:764 | 通用开隧道（token=""，open_url=local_addr） | GenericTunnelCard（generic-tunnel-card.tsx:33-50） | 有 |
| `tunnel_status` | gui-contract.md:761 | 两侧状态快照（单会话形状） | 状态卡（remote-access-view.tsx:47-69、181-222） | 有 |
| `tunnel_serve_start(target)` | gui-contract.md:762 | 白名单累积 + 开启受理 | TunnelServeCard（tunnel-serve-card.tsx:35-50） | 有 |
| `tunnel_serve_stop` | gui-contract.md:763 | 关受理、白名单保留 | TunnelServeCard（tunnel-serve-card.tsx:52-67） | 有 |
| 事件 `tunnel_status` | gui-contract.md:770 | 任一侧变更主动 emit | `ipc.onTunnelStatus`（remote-access-view.tsx:60-63） | 有 |

结论：**§19.1 命令表 GUI 全覆盖**；缺口全部在能力/交互层（§2-§4），不在命令有无层。

## 2. 差距矩阵（场景：「我(owner)把本机 http/ws 服务分享给指定 peer」全流程）

| # | 步骤 | 现状 | 证据（file:line） | 差距 |
|---|---|---|---|---|
| S1 | 被访侧开放本机服务端口 | 部分已有 | tunnel-serve-card.tsx:35-50 → tunnel.rs:97-103 → config.rs:127-135（add_allow 累积） | 无法批量多端口、无 `--max-concurrent` 调节、白名单只显示逗号串不可管理 |
| S2 | **授权指定 peer** | **缺失** | 准入无 peer 维度：config.rs:150-164 `authorize(target)` 只查 开关→白名单→并发；responder.rs:280-282 peer 仅入审计；GUI 无任何 per-peer 授权控件 | 核心缺口：现语义=开放即对**所有**能握手的 peer 生效，无法「只给指定 peer」 |
| S3 | 对端（访侧）发起连接 | 已有 | generic-tunnel-card.tsx:33-50 → visit.rs:249-267 `tunnel_open` | peer 靠手输 base58（generic-tunnel-card.tsx:86-91），无好友/邻居选择；**GUI 访侧单会话槽位**（visit.rs:87-89 `Mutex<Option<ActiveSession>>`，start 先 stop 旧会话 visit.rs:123-131）→ 同时只连一个服务 |
| S4 | 得到对端使用说明 | 部分已有 | open_url 展示+复制（remote-access-view.tsx:181-222、204-220）；`tunnel_open` 的 open_url=local_addr 本身（visit.rs:170 audit::open_url；url.rs:36-51 token 恒空） | 无 ws:// 地址派生提示；无「分享说明」一键文本（告诉对端：用 GUI 哪个页面+我的 peer+端口）；`tunnel_open_dsh` 只收 http:// 前缀（url.rs:15-17），粘贴 ws:// URL 不支持 |
| S5 | 查看状态/健康 | 部分 | 访侧：事件驱动+近 3 条审计行（remote-access-view.tsx:47-69、194-203）；被访侧：仅 enabled/allow/activeSessions 计数（tunnel-serve-card.tsx:130-148；tunnel.rs:86-93、148-156） | **被访侧会话明细（谁在连、peer、字节、outcome）无任何 IPC 暴露**：responder 的 `TunnelAudit` 快照存在（responder.rs:66-68、143-152）但 tunnel.rs:42-56 `install()` 未保存 audit 句柄，无命令可读 |
| S6 | 撤销分享 | 部分/不可用 | 被访侧只能全停：tunnel-serve-card.tsx:52-67 → tunnel.rs:105-109（`set_enabled(false)` 白名单保留）；`TunnelGate::set_allowlist` Rust API 存在（config.rs:112-120）但**无删除单端口的命令**；访侧：**无关闭按钮**——契约即无关闭命令（remote-access-view.tsx:36 注释「契约 §7 无关闭命令」；visit.rs:181-188 stop 仅被内部 start 顶替/退出收尾 192-204 调用） | 撤单端口不可做；访侧会话开完即无法从 GUI 主动关（只能被下一个会话顶替或退出应用） |
| S7 | 故障可观测 | 部分 | 命令失败进错误盒+toast（remote-access-view.tsx:159-170；tunnel-serve-card.tsx:150-160）；反代 501/502 人话只写进浏览器响应体（local_proxy/conn.rs:23-25、74-76、167-186），错误码入访侧审计（conn.rs:44-50） | 运行中失败（dial_failed/busy 等）只体现为浏览器 502 页，GUI 无运行时错误横幅；拒绝明细在被访侧不可见（同 S5） |

## 3. 特别判定（代码级结论）

### (a) ws 经回环反代可用性 — **可用，透传已实现**
- 升级识别：head.rs:99-105 `is_websocket_upgrade`（Upgrade:websocket + Connection 含 upgrade，大小写不敏感）；conn.rs:21 判定、conn.rs:86-90 分路。
- 升级路径：conn.rs:94-126 `upgrade_path`——101 应答头透传回本地，101 后 `copy_bidirectional` 裸字节双向泵（conn.rs:122-125）；101 前余量字节不丢（conn.rs:99-104、115-121）；升级被拒则排干至 EOF（conn.rs:110-114）。
- 头处理：升级请求保留 Connection/Upgrade 不改写（head.rs:87-95 `apply_hop_by_hop(websocket)`）；Host/Origin/Referer 同样重写为 `127.0.0.1:<target>`（head.rs:72-81），避免被目标按 authority 拒升级。
- 协议层数据面为纯字节（docs/protocol/specs/tunnel.md §2.4/§3.2，wire 层无 HTTP 语义），ws 帧不受限。
- 残余缺口（非能力缺口）：`TunnelOpenReport.openUrl` 恒 http://（types.rs:14-15、tunnel.rs:116-126）；ws 客户端需自行拼 `ws://127.0.0.1:<local_port>/<path>`，GUI 未提示；`tunnel_open_dsh` 严格 http:// 前缀（url.rs:15-17），ws:// 启动 URL 不可走 DSH 入口（通用入口不受影响）。

### (b) serve 侧准入粒度 — **全局按目标，非按 peer；不能限定单 peer**
- `TunnelGate::authorize(target)` 签名即无 peer：config.rs:150-164（开关→白名单精确匹配→并发许可三连判）；白名单键是目标端口字符串（config.rs:66-70）。
- responder 持有握手 PeerId 但只用于审计：responder.rs:280-282（`handle_inbound` 传 peer）→ responder.rs:143-152（`audit.record` 的 `peer_id` 字段）；票据本身无身份字段且禁止采信（tunnel.md §2.2:61-62）。
- 结论：`tunnel_serve_start` 开启后，**任何能与本机节点完成底座握手并开 `/p2p-base/tunnel/1` 流的 peer 都可访问全部白名单端口**（底座 `Node::handle_protocol` 无 peer 过滤，crates/p2p/src/node.rs:77）。要「指定 peer 可用」必须扩展 Rust 门禁（gate 增加 peer 维度 + responder 把 peer 传入 authorize），当前无此 API，GUI 也无此命令。

### (c) 多服务并存 — **被访侧可并存；GUI 访侧不可**
- 被访侧 YES：allowlist 是 HashSet，`add_allow` 累积（config.rs:127-135），`tunnel_serve_start` 每次加一端口并保持开启（tunnel.rs:63-69，重复调用即多项，gui-contract.md:762）；CLI `--target` 可多次（serve.rs:25-26、51-71）。
- GUI 访侧 NO：单会话槽位 `session: Mutex<Option<ActiveSession>>`（visit.rs:87-89），`start()` 先 `stop()` 旧会话（visit.rs:123-131 注释「先关旧会话保证幂等」）；状态契约也是单会话形状 `active/localAddr/target`（types.rs:26-37）。CLI 访侧每进程一个 target（connect.rs:103 `LocalProxy::bind(cfg.target_port,...)`）。
- 推论：「owner 同时分享 8080 给 A、3000 给 B」被访侧数据面支持，但 GUI 访侧同一时刻只能挂一条隧道；且 DSH 卡与通用卡共享同一槽位，互相顶替。

### (d) 服务健康态可见性 — **访侧基本可见；被访侧只有计数，明细断供**
- 访侧：`tunnel_status` 命令 + `tunnel_status` 事件（visit.rs:207-212 emit；remote-access-view.tsx:60-63 订阅），会话审计八字段（types.rs:42-59），UI 显示 localAddr/openUrl/target/在途数+近 3 条（remote-access-view.tsx:187-203）。
- 被访侧：仅 `TunnelServeStatus{enabled, allow, activeSessions}` 计数面（tunnel.rs:24-28、86-93、148-156）。responder 侧明细审计（peer_id/target/bytes/outcome，八字段闭集）在 `TunnelAudit` 里真实存在（responder.rs:52、66-68；audit 记录于 responder.rs:143-152），但 `TunnelServeSlot::install()` 装配后丢弃 responder/audit 句柄（tunnel.rs:42-56），无任何 IPC 命令读取 → GUI **看不到谁在访问自己**。
- 拒绝/故障人话（501/502 reason）只进浏览器响应体（conn.rs:167-186），GUI 错误盒仅覆盖命令同步失败路径。

## 4. GUI 交互设计提案

### 4.1 被访侧「分享服务」向导（新增卡片或改造 TunnelServeCard）
1. **选服务**：端口输入（可重复添加多个）+ 协议标签（http / ws / 都行——仅展示用途，数据面不区分）+ 可选并发上限（默认 16，config.rs:32-37 口径展示）。
2. **选对象**：每条分享绑定授权 peer——从好友列表选择（复用 contacts 数据与 `PeerIdField` 即时校验样板，allowlist-panel.tsx:43-50、206-214）或留空=「任何 peer」（现语义，明示风险文案）。
3. **确认开放**：调 `tunnel_serve_start`（+ 新授权命令，见 4.3 后端栏）；成功 toast + 列表新增一行（allowlist-panel.tsx:147-159 的「成功即收起表单、新行可见」样板）。
4. **管理/撤销**：已分享清单表格（peer × 端口 × 授权时间 × 状态），单行「撤销」走全站二次确认（allowlist-panel.tsx:162-176 `useConfirm` 样板）；整页「停止受理」= 现 `tunnel_serve_stop`。
5. **生成对端说明**：一键复制分享文本：「在 GUI 远程访问页 → 通用开隧道：端口 `<port>`、Peer `<我的 peer>`」（我的 peer 取自 profile/节点信息，纯前端拼接）。

### 4.2 访侧「使用分享」增强（改造 GenericTunnelCard）
- peer 输入升级为好友选择器 + 手输兜底（PeerIdField + `PeerNameCell` 显示可读名，allowlist-panel.tsx:27、60）。
- 成功后展示三行：`http://`、派生的 `ws://127.0.0.1:<local_port>`（纯字符串替换 scheme，提示「ws 客户端直接连此地址」）、分享端口与对端 peer。
- 多服务并存：卡片列表化（每条会话一行：target/localAddr/在途连接/字节），替代现单会话状态卡；需后端多会话支持（见下）。
- 增加显式「断开」按钮（调新 `tunnel_close`，或在多会话重构时随行提供）。

### 4.3 缺口 → 实现建议（两栏分列）

**纯前端可做（零 src-tauri 改动）**

| 缺口 | 建议 |
|---|---|
| S2 部分/S4 分享说明 | 分享文本一键复制、ws:// 地址派生提示、协议标签展示 |
| peer 手输体验 | 好友选择器 + PeerIdField 校验样板移植（allowlist-panel.tsx:43-50、206-214） |
| S1 白名单展示 | 逗号串改表格/徽章列表（数据已在 `serve.allow`，ipc-types.ts:778-781） |
| S5 访侧明细 | 审计行扩为完整表格 + 错误码人话映射（六值闭集，gui-contract.md:804-806） |
| S7 运行时错误 | 消费现有 `tunnel_status` 事件中 outcome≠open 的终态记录做横幅提示（事件已必发，gui-contract.md:770） |

**需动 src-tauri/后端（单列，勿混入前端项）**

| 缺口 | 改动 | 依据 |
|---|---|---|
| S2 指定 peer 授权 | `TunnelGate` 增加 peer 白名单（`authorize(target, peer)`），responder.rs:95 把已有 peer 传入；新增 IPC `tunnel_serve_allow_peer/deny_peer`；`TunnelServeStatus` 加 `peers` 字段（契约 §19.2 加法） | config.rs:150-164、responder.rs:95-108 |
| S6 撤单端口 | 新增 IPC `tunnel_serve_remove(target)`（内部走已存在的 `TunnelGate::set_allowlist`，config.rs:112-120），或 set 全量替换命令 | 现仅 add 无 remove（tunnel.rs:63-69） |
| S5 被访侧明细 | `TunnelServeSlot::install()` 保存 `responder.audit()` 句柄（responder.rs:66-68）；新增 IPC `tunnel_serve_sessions()` 或并入 `tunnel_status.serve`，复用 `TunnelSessionAudit` 八字段映射（types.rs:61-85 现成） | tunnel.rs:42-56 现丢弃句柄 |
| S3/S6 访侧多会话+关闭 | `TunnelState.session` 单槽改 `HashMap<uid, ActiveSession>`；新增 IPC `tunnel_close(sessionId 或 target)`；`TunnelStatusReport` 由单会话字段改 `sessions[]` 顶层形状（契约 §19.2 变更，需按 §6 版本纪律做加法或升版） | visit.rs:87-89、123-131；types.rs:26-37 |
| 可选：并发上限 | `tunnel_serve_start` 增加可选 `maxConcurrent` 参数（Rust 侧 `TunnelServeConfig` 已支持） | serve.rs:31-32、config.rs:37 |

### 4.4 交互样板复用清单（allowlist-panel.tsx）
表格信息优先+展开式表单（:76-77、193-201、204-256）· PeerIdField 即时校验+聚焦首错（:43-50、140-146）· 成功 toast+收起回列表（:156-158）· 破坏性撤销二次确认（:162-176）· 操作失败显式露出不吞（:125-138）· PeerNameCell 可读名（:60）。

## 验收对照
- 四节齐全 ✔；差距矩阵 7 步骤（≥6）全带 file:line ✔；§3 四判定均代码级结论 ✔；提案区分纯前端/需动后端两栏 ✔。
- 工具预算：28/30 次内完成，零生产代码改动，未建 worktree，未跑构建。
