# 配置集中化盘点：设置页之外的配置信息（2026-09-17）

> 目标（用户指令）：「remote-access 这些都是配置信息，都应该放配置页面」。
> 本文档盘点全部散落在设置页之外的配置面，给出「迁移设置页 / 留在业务页」判定与迁移映射。
> 真值源：代码 file:line 证据；设置页边界 = apps/gui/src/views/settings/。
> 扫描口径：GUI 侧 97 项 + Rust/CLI/环境侧约 85 项（详见 §3-§5）。

## 1. 设置页现状（基线边界）

设置页六区（settings-view.tsx:58-74）：档案(Profile)、身份(Identity)、外观(Appearance)、
网络(Network+Advertise)、服务(Services)、关于/文档/LLM 入口。

持久化载体 `gui-config.json`（app 数据目录，ConfigStore 原子写，config.rs:12），
GuiConfig 11 字段：quicPort、tcpPort、enableMdns、dataDir、bootstrap、relayAddrs、
advertisedAddrs、observationPort、observationAddrs、lanOnly、authzDefaultRole。

**GuiConfig 与设置页表单 schema（config-schema.ts:45-73）字段集完全一致，无漂移**。
但 UI 暴露面有 3 个缺口（GUI 扫描结论）：
1. `authzDefaultRole`：设置页无控件（config-schema.ts:56-58 注释明示），真实编辑入口
   在通讯录好友角色对话框（friend-role-dialog.tsx:213-228，走 authz_default_role_save）。
2. `bootstrap` / `relayAddrs`：设置页不重复编辑（advertise-card.tsx:14-15 注释），
   编辑入口外移到发现页 rendezvous 地址簿（add-address-dialog.tsx:56-66）与
   中继页地址卡（relay-config-card.tsx:105-110）。
3. `dataDir`：设置页只读展示（identity-card.tsx:55-61）。

## 2. 判定原则

- **持久化策略/默认值/地址簿 → 迁设置页**（用户裁决：配置应集中）。
- **一次性运行态操作/对象级操作 → 留业务页**（连接目标、批准/拒绝、启停按钮、
  按好友逐个绑角色——这些是"动作"不是"配置"；业务页可从设置读默认值）。

## 3. GUI 侧散落配置面（97 项，扫描详见子代理报告归档）

### 3.1 迁移候选（真配置，用户裁决应集中）

| 配置项 | 现位置 | 现状 | 迁移目标 |
|---|---|---|---|
| RD 审批开关（新会话需审批） | remote-desktop-card.tsx:63-67 | 会话态开关，非落盘 | 设置页·远程访问区（GuiConfig 新增 rdRequireApproval，卡片读默认值） |
| RD 质量档 fps（1-60） | remote-desktop-card.tsx:142-150 | 会话态 | 同上（rdFps） |
| tunnel serve 白名单 | tunnel-serve-card.tsx:134-141（表只读 tunnel-allow-table.tsx） | **进程内存态，重启即失**（p2p-tunnel config.rs:61 TunnelGate，无落盘点） | 设置页·远程访问区 + 新增持久化（GuiConfig tunnelServeAllow） |
| rendezvous 地址簿（bootstrap） | discovery/add-address-dialog.tsx:56-66 | 已持久化（configSave） | 设置页·网络区补齐入口（业务页可留快捷跳转） |
| 中继地址 relayAddrs | relay-config-card.tsx:105-110 | 已持久化 | 设置页·网络区补齐入口 |
| 好友默认角色 authzDefaultRole | contacts/friend-role-dialog.tsx:213-228 | 已持久化（唯一的设置页外 GuiConfig 写点） | 设置页·服务或身份区补齐入口 |
| FTP 服务配置（root/账号表/authz） | **GUI 无任何入口**，仅 ftp.json 手改（apps/cli/src/ftp_serve.rs:77） | 文件配置 | 设置页·服务区新增（W2） |
| 静态对端地址簿 static-peers.json | **GUI 无入口**（crates/p2p/src/static_peers.rs:94，0600 原子写） | 文件配置 | 设置页·网络区新增（W2，含安全评审） |

### 3.2 保留在业务页（运行态/对象级操作，非配置）

- 远程访问页：DSH URL、被访 PeerId、端口输入、好友选择器、连接/断开、批准/拒绝、
  开启/停止被控（remote-desktop-card.tsx、dsh-open-card.tsx、generic-tunnel-card.tsx）
- events 页筛选/暂停/导出、peers 搜索与手动拨号、diagnostics 暂停/清理（会话内）
- llm-share 全部 24 项：offer/provider/分享/borrow/allowlist 是**业务对象 CRUD**，
  非应用配置（provider apiKey v13 起已入后端存储）
- contacts：好友角色绑定（按对象逐个设）、好友增删改、群管理
- ACP endpoint 表单/抽屉、agent 会话配置下发、acp-manage 工作区、agents CRUD

### 3.3 安全注意点（独立整改候选）

- ACP endpoint 的 token/adminToken 以**明文**存 localStorage
  （`p2p-gui-acp-endpoints`，acp/endpoint-storage.ts:7,36-57）——应迁后端存储
  （参照 llm-share provider v13 迁移先例 provider-configs.ts:108-173）。

### 3.4 localStorage 偏好键（8 类，保持现状）

主题、语言、已退群开关、会话置顶/免打扰、跳过更新版本、通讯录折叠、
ACP endpoint 存档、endpoint-meta 权限策略——UI 偏好类，不属"配置集中"范畴。

## 4. Rust/CLI/环境侧（约 85 项，GUI 外配置）

### 4.1 环境变量（13 个产品读取点）

P2P_CONTROL_PORT（GUI 控制通道 7819，control/server.rs:49）、P2P_METRICS_LOG_SECS、
RUST_LOG、REPAIR_ROOTS；HOME/XDG 系仅目录推导（RD 文件根 ~/Downloads/RD 等）。

### 4.2 文件配置（可手改价值排序）

1. `gui-config.json`：bootstrap/relayAddrs/dataDir/authzDefaultRole 字段 GUI 编辑入口
   不在设置页（见 §3.1，本次迁移对象）
2. `services.json`（p2p-service store.rs:21，11 个 service_id 闭集）——GUI services-card
   已覆盖开关面
3. `ftp.json`（root+accounts+authz）——**GUI 零入口**（§3.1 迁移候选）
4. `static-peers.json`——**GUI 零入口**（§3.1 迁移候选）
5. `authz/roles.json`+`bindings.json`（角色/绑定，GUI 通讯录覆盖操作面）
6. `chat/serve.json`（记忆端口）、`acp-*.json`（ACP 桥，独立二进制面）

### 4.3 硬编码默认值（约 40 项，代码常量为主）

RD（fps 15/1-60、idle 15s、剪贴板轮询 500ms、fs_root ~/Downloads/RD、
CLI 审批 false vs GUI true）、QUIC keepalive 10s/idle 30s、TCP 连接 5s、
relay 配额（每 peer 8 链路/32 电路/1MiB/s）、relay 电路 TTL 300s、
tunnel 并发 16、mDNS TTL 15s、chat ACK 10s、更新源 GitHub API 硬编码。
→ **不建议迁设置页**（服务端/协议级常量）；确有用户价值的（RD fs_root、
fps 上限）经 CLI 参数已可达（p2pctl rd host --fs-root/--fps/--codec）。

### 4.4 CLI 面

`p2pctl config get/save` 整读写 GuiConfig；`rd host`/`tunnel serve`/`vdrive serve`/
`chat serve` 带 --quic-port/--tcp-port/--data-dir/--bootstrap 等；
acp-agent 独立二进制有完整 CLI+JSON 双配置面（apps/acp-agent/src/cli.rs:9-69）。
→ CLI 与 GUI 设置页共享 gui-config.json，天然同源，无需迁移。

### 4.5 tauri.conf.json

窗口 1280×800、updater endpoint+pubkey、CSP null、identifier com.p2p.console
——构建期配置，重打包生效，不属运行时设置页范畴。

## 5. 迁移映射（W1-W3 实施建议）

| 波次 | 内容 | 涉及 |
|---|---|---|
| W1 | GuiConfig 扩展（rdRequireApproval/rdFps/tunnelServeAllow，serde camelCase + 字段级默认，config.rs 模式）+ 设置页新增「远程访问」区 + rd 卡/serve 卡改读默认值 | ipc-types、config-schema、settings 新卡、rd.rs/tunnel.rs 装配读配置、cli-parity 新命令登记 |
| W2 | bootstrap/relayAddrs/authzDefaultRole 设置页入口补齐（三处外移编辑口归拢）；ftp.json 与 static-peers.json 的 GUI 面新增（含安全评审） | settings 网络区/服务区扩展 |
| W3 | ACP endpoint token 明文 localStorage → 后端存储（参照 provider v13 迁移先例） | acp/endpoint-storage、后端存储面 |

业务页改造原则：卡片保留启停/审批/连接等**动作按钮**，策略与默认值读设置页；
原散落控件删除或改为跳转设置页的链接。

## 6. 待用户裁决

1. §3.1 的迁移清单是否认可（尤其「运行态操作留业务页」的边界）。
2. tunnel serve 白名单随迁移新增持久化（现状重启即失）——是否顺带做。
3. W2 的 ftp/static-peers GUI 面是否纳入（涉及账号表与静态对端的安全暴露面）。
