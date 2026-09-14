# 服务总控事实盘点（2026-09-13，只读）

范围：/Users/imeepos/ext512/p2p 主树当前工作副本。只盘点事实，不做设计决策。所有断言带 file:line（相对仓库根）；查不到写「未找到」。
开关语义约定：「隐式」= 无布尔开关字段，由列表非空/文件存在/进程存活等条件推导；「会话态」= 仅内存，重启回落默认。

## 表1 服务清单

| 服务面 | 现有开关机制（file:line） | 开关状态存储与持久化 | 当前操作面 | 出厂默认 |
|---|---|---|---|---|
| mdns 局域网发现 | GuiConfig.enable_mdns（GUI apps/gui/src-tauri/src/types.rs:28-29；CLI apps/cli/src/types.rs:40-41；NodeConfig crates/p2p/src/lib.rs:31）；消费点 GUI state.rs:298 / CLI daemon.rs:26 / 底座 assembly.rs:180-187 | gui-config.json 持久化，原子写（GUI config.rs:12,118-140；CLI config.rs:57-72 经 store::save_config）；不热更，重启生效 | GUI 设置页开关（views/settings/network-card.tsx:131-138）；CLI p2pctl config save 整份 JSON（apps/cli/src/config.rs:15-20）；旗标 p2p-cli node --no-mdns（crates/p2p-cli/src/cli.rs:72）、p2pctl tunnel serve --no-mdns（apps/cli/src/tunnel/serve.rs:43-44） | true（GUI config.rs:36-38,55；CLI types.rs:72） |
| rendezvous 注册（客户端） | 隐式：bootstrap 列表非空才接线（assembly.rs:188-198,208-210；lib.rs:29-30「空则跳过」） | gui-config.json bootstrap 持久化（types.rs:33-35） | GUI 设置页地址行（views/settings/config-schema.ts:50,66,83）；CLI p2pctl config save；p2pctl tunnel serve --bootstrap（tunnel/serve.rs:45-47）；acp-agent --a2a-bootstrap（apps/acp-agent/src/cli.rs:66）；lan_only 装配期剥离（assembly.rs:126-132） | 2 台云端（GUI config.rs:15-20） |
| rendezvous 服务端 | 每节点装配恒注册 RendezvousServer（assembly.rs:82-84）；公共策略开关 rendezvous_public_only（lib.rs:49-50）；GUI/CLI 的 GuiConfig 均未含此字段（未找到暴露面） | 进程态，不持久化 | 独立部署面仅旧 p2p-cli bootstrap --allow_private（crates/p2p-cli/src/cli.rs:20,47） | rendezvous_public_only=false（lib.rs:73）；即每个 GUI/daemon 节点默认也受理他人注册（拒全不可路由除外） |
| relay 客户端（降级链 2/3 跳） | 隐式：relay_addrs 非空（lib.rs:36-37；assembly.rs:68,75）；空=降级链止于直连；lan_only 剥离（assembly.rs:128；CLI daemon.rs:29-34 直接短路） | gui-config.json relayAddrs 持久化（types.rs:36-37） | GUI 设置页（config-schema.ts:51,67）；CLI p2pctl config save | 2 台云端（GUI config.rs:23-28） |
| relay 服务端（bootstrap 角色） | 进程装配即开：RelayServiceImpl::spawn（crates/p2p-relay/src/service.rs:110-114）；生产装配仅在旧 p2p-cli（crates/p2p-cli/src/relay_serve.rs:68-71） | 进程态 | 仅 p2p-cli bootstrap 子命令（crates/p2p-cli/src/cli.rs:20）；GUI/p2pctl 无此面（未找到） | 随命令显式启动 |
| 地址观测（出站上报） | 隐式：observation_addrs 非空（lib.rs:42-43；assembly.rs:54-66，耗尽告警+降级置位） | gui-config.json observationAddrs 持久化（types.rs:42-43） | GUI 设置页（config-schema.ts:54,70）；CLI p2pctl config save | 1 台云端（GUI config.rs:31-33） |
| 观测反射口（UDP 服务） | observation_port=Some 才启（lib.rs:40-41；assembly.rs:45-51）；lan_only 强制 None（assembly.rs:130） | gui-config.json observationPort 持久化（types.rs:40-41） | GUI 设置页（config-schema.ts:53,69,86）；CLI daemon 消费（daemon.rs:39-41）；p2p-cli bootstrap --observation-port（crates/p2p-cli/src/cli.rs:44） | None=不开（GUI config.rs:60） |
| llm-share 借出 serve（闸1/闸2 宿主） | 隐式：node_start 装配，offer.json status=live 且模型-provider 唯一匹配才注册双 handler（state.rs:121 → serve/mod.rs:108-137,148-155,169-170）；无布尔总开关 | offer.json/allowlist.json/provider/密钥文件持久化（gate.rs:2-5；serve/mod.rs:219-231）；assembled 为会话态（serve/mod.rs:79-84） | GUI llm-share 页九命令+8 条 v13（views/llm-share/backend.ts:25-45；Tauri 侧 llm_share/commands.rs:26-28,159-161,204-206）；CLI p2pctl llm-share 域（apps/cli/src/llm_share/mod.rs:23-57） | 未发布 offer=不装配（assembled:false 常态，serve/mod.rs:5-7） |
| llm-share 借入 borrow | 无开关，按次调用 | —— | GUI borrow 面板（backend.ts:30）；CLI llm-share borrow（apps/cli/src/llm_share/mod.rs:40-41） | —— |
| a2a agent 发布与 private task | 独立 acp-agent 进程承载 /a2a/1（main.rs:36-38）；进程级开关 --a2a-disabled（cli.rs:69；config.rs:81,108 默认 false）；agent 级 enabled+visibility（apps/acp-agent/src/a2a/task.rs:63-67） | AgentStore/GrantStore 落 acp-agent 数据目录（task.rs 构造，a2a/agents、a2a/grants） | GUI agents 页经本机 admin HTTP（views/agents/agents-page.tsx:32,103,119，凭据读 ~/.dsh/acp/local-agent.json，acp_descriptor.rs:2）；CLI p2pctl a2a list/publish/unpublish/allow/disallow（apps/cli/src/a2a/mod.rs:18-29） | 进程不随 GUI/daemon 拉起（拉起点未找到）；a2a_disabled=false |
| ACP agent 托管（/dsh-acp/1 桥+子进程） | 无总开关；子进程 command 默认 "pnpm dsh --profile acp"（cli.rs:25-27；config.rs:113-115）；资源闸 ConnGate 每peer/总数（apps/acp-agent/src/gate.rs:31-45）；--admin-disabled 只关管理面（cli.rs:58-60） | 策略表 acp-policy.json（apps/cli/src/acp/mod.rs:44 提及）+ 分享台账 | CLI p2pctl acp allow/deny/list/share/console/status（apps/cli/src/acp/mod.rs:29-42）；GUI acp-manage 页 workspace/状态/会话（views/acp-manage/，经 use-admin-endpoint.ts）；GUI 无策略表 allow/deny 面（未找到） | admin_disabled=false（config.rs:105） |
| ACP console 泵（GUI 进程内） | GUI 启动即 Pump 启动，无开关（console/mod.rs:2,38） | 会话态；数据目录 app_data/acp-console-data（console/mod.rs:24） | 无启停操作面（状态只读 acp_console_status，ipc.ts:75） | 恒开（失败转 disconnected 不阻断，console/mod.rs:2-4） |
| tunnel 被访（serve） | 会话态闸：GUI slot 默认关（tunnel.rs:41-56），start/stop 显式开关（tunnel.rs:63-76,97-109）；CLI 进程活=enabled（tunnel/serve.rs:84-88）；闸三连判 开关→目标白名单→并发（crates/p2p-tunnel/src/config.rs:152-167；「allowlist 默认空、开关默认关=双默认全拒」:13-16） | 会话态不持久化，重启回落关闭（tunnel.rs:1-2）；白名单内存态（add_allow config.rs:127-135） | Tauri 命令已注册（lib.rs:124）但前端无任何调用（未找到，见问题2）；CLI p2pctl tunnel serve --target/--allow（apps/cli/src/tunnel/mod.rs:13-14；serve.rs:22-29） | enabled=false + 白名单空 |
| tunnel 访问（visit/open） | 无开关，按次开隧道 | 活跃会话内存态 | GUI remote-access 页 tunnelOpenDsh/tunnelOpen（tunnel.rs:118-137；views/remote-access/remote-access-view.tsx:74,155；generic-tunnel-card.tsx:37,93-101）；CLI p2pctl tunnel connect（tunnel/mod.rs:16） | —— |
| repair 桥/票据 | 独立 bin repair-helper：serve 受理 /repair/mcp/1 票据校验前置（crates/repair-helper/src/main.rs:3-5,40-41）；mint-ticket 显式命令（main.rs:43） | 票据台账+审计 JSONL（main.rs:57-59） | 无 GUI/p2pctl 面（apps/cli/src/cli.rs 命令域无 repair）；仅 repair-helper serve/mint-ticket | 显式命令启动 |
| chat/IM 单聊 | 无开关，随节点常驻装配（state.rs:110-117） | 消息/好友库落 data_dir（crates/p2p-chat/src/store*.rs） | GUI chat 页（chat.rs:169 chat_send）；CLI p2pctl chat 域（apps/cli/src/cli.rs:43-47） | 恒随节点 |
| 群聊 | 无开关，随节点 | 群库 group_store.rs | GUI group.rs:16-146 命令面；CLI p2pctl group 域（cli.rs:48-52） | 恒随节点 |
| 文件传输（聊天附件） | 无独立开关：收随消息（crates/p2p-chat/src/wire.rs:265-272 receive_media），发随 chat/group send | 媒体落 app 数据目录（state.rs:76-77） | GUI chat_media_file（chat.rs:189）/导出 media_export.rs:150-153/群媒体 group.rs:146；CLI chat/group media 子命令 | 恒随消息面 |
| update 更新 | 无自动更新开关，手动检查型 | 本地版本比对，无持久化开关 | GUI update_check/update_open_release_page+应用内下载安装（update.rs:113-141；契约 §13，ipc.ts:294-326；views/update/）；CLI p2pctl update check/open（apps/cli/src/update/mod.rs:17-22，无下载安装） | —— |
| GUI 控制通道（p2pctl gui 域） | 127.0.0.1 HTTP+token 鉴权（control/server.rs:37-45）；invoke 只读白名单（control/invoke_allow.rs:8-14） | 进程态 | CLI p2pctl gui status/screenshot/record/navigate/invoke（cli.rs:78-82） | 端口策略启动即绑 |

## 表2 对外能力闸 vs authz

| 能力（远端 peer 用本机能力） | 当前判定机制（file:line） | 已走 p2p-authz | 未接原因/设计预留 |
|---|---|---|---|
| llm 借出代理（闸1 身份准入） | GUI 装配注入 with_gate1_authz（serve/proxy.rs:72,108）→ serve/authz.rs:16-29 → crates/llm-share-proxy/src/gate1.rs:29 check_borrow；判定点 server.rs:119,196-201；读失败=拒 | 是（llm.borrow） | ——；crate 层未注入 gate1 时回落旧 allowlist（server.rs:66-67,197；tests/authz_gate.rs:254「回滚路径」），allowlist 转只读归档（serve/authz.rs:2-6） |
| llm 借出模型白名单（闸2） | AllowlistGate.admits 条目存在且未过期（serve/gate.rs:52-62）；server.rs admit 序 | 否（有意保留） | 对象级模型白名单非 peer 级授权，设计明示保留（docs/design/authz-role-design.md:119） |
| a2a 公开卡 discover | stream.rs 相可见性过滤；peer_authz.rs:9-10 明示 discover 路径不经本闸 | 否（预留） | 「公开池本就公开（现状无判定，预留）」（authz-role-design.md:60）；§14 开放问题未给时间表 |
| a2a private task | 双查：peer 级 check(a2a.invoke) 先拒（a2a/peer_authz.rs:29-40，读失败=拒）+ agent 级 grants.is_granted（a2a/task.rs:63-75）；owner 两查皆免（task.rs:73） | 是（a2a.invoke） | —— |
| acp 会话建立 | 瀑布（apps/acp-agent/src/session.rs:152-165）：①策略表命中 ②token 兑换 ③非 owner 须 check(acp.session)（session.rs:194-201）④其余 fail-closed；拒绝只入审计不回 wire（session.rs:196-198） | 是（acp.session） | ——；owner 不进 authz（红线1） |
| acp execute 类 ask | 权限瀑布前置（apps/acp-agent/src/permission.rs:85-96 decide_gated）；router/window.rs:107-121 check(acp.execute)，Deny 本地直拒不弹窗 | 是（acp.execute） | ——；owner scope 直通（window.rs:109-111） |
| chat 单聊消息入口 | crates/p2p-chat/src/wire.rs:249-298：帧校验后直接落库+广播（:293 append_message），无好友校验、无 authz（全 crate 无 authz 引用） | 否 | 设计预留：「好友即通（现状无判定，权限预留）」（authz-role-design.md:58；§14 开放问题4 :186）；key 已登记 crates/p2p-authz/src/permissions.rs:15 |
| chat 附件（媒体接收） | wire.rs:265-272 随消息 receive_media，同上无判定 | 否 | chat.attachment 同上预留（authz-role-design.md:59 语境；§14 :186） |
| 群聊消息/群媒体 | group_send.rs/group_wire.rs 全 crate 无 authz 引用（未找到） | 否 | 九权限闭集无群聊粒度 key（permissions.rs:78-86），设计登记表未覆盖群维度 |
| tunnel 被访 | authorize 三连判=开关→目标白名单→并发（crates/p2p-tunnel/src/config.rs:152-167）；responder 持 peer 身份（responder.rs:71,288 仅要求已认证）但不参与判定 | 否 | 白名单是目标级（127.0.0.1:port）非 peer 级；无 tunnel.* 权限 key；收口议题已挂账（TODO.md:26「tunnel visit 是否入 authz?」） |
| repair 票据 mint | mint 前置 check_bridge（repair-helper main.rs:144 → authz_gate.rs:30-48），scope→key 映射 :13-19；读失败=拒+authz.denied 审计（:36-47） | 是（repair.diag/repair.fix） | —— |
| repair serve 入站 | 票据本体校验前置：一次性/签名/绑对端（main.rs:3-5,40-41） | 否（非 authz 层） | authz 闸置于 mint 侧，serve 侧信任票据语义（docs/ops/repair-runner-integration.md:81-82） |
| 底座连接门禁 | swarm 入站/出站统一门禁 gate_allows（crates/p2p-swarm/src/swarm/listen.rs:64-78；dial.rs:189-193；registry.rs:30-36），拒绝计数 gate_denials（metrics.rs:89-91） | 否 | 扩展点已留（ConnectionGate trait crates/p2p-swarm/src/lib.rs:83；node.rs:82-84 set_gate）但 GUI/CLI 生产代码从未装配（apps 下 set_gate 调用未找到）→ 默认放行所有已认证 peer |
| echo/ping 探测 | p2pctl daemon 注册 EchoHandler 恒应答（daemon.rs:81-84）；swarm ping 内建 | 否 | 调试协议；任意 peer 可探测在线/RTT（未找到任何判定） |

## 表3 GUI/CLI 开关操作面差距

| 服务 | GUI 操作面 | CLI（p2pctl）操作面 | 差距备注 |
|---|---|---|---|
| mdns | 有：network-card.tsx:131-138 | 有：config save（cli config.rs:18-19）；config get 回显 :92 | —— |
| lanOnly | 前端有开关（network-card.tsx:140-147；config-schema.ts:55,71,88）但 Rust 后端 GuiConfig 无字段（apps/gui/src-tauri/src/types.rs:21-48）→ 保存丢弃、启动不消费（state.rs:293-307 无 lan_only） | 有：cli types.rs:54-56 + daemon.rs:28 消费；config 回显 :99 | GUI 开关当前无效（详见问题1） |
| bootstrap/relay/observation/advertised | 有：config-schema.ts:50-54,66-70 + advertise-card.tsx | 有：config save/get :94-98 | —— |
| llm-share offer/allowlist/share/provider/borrow | 有：llm-share 页 backend.ts:25-45（offer publish :25、allow :28、share :41-44、provider :38-40） | 有：llm-share 域 mod.rs:23-57 | CLI offer 仅 publish/show，无 unpublish/撤销声明（offer.rs:16-21，见问题5） |
| a2a 发布/可见性/授权 | 有：agents 页经 admin HTTP（agents-page.tsx:103-124 unpublish/visibility） | 有：a2a 域 mod.rs:18-29 | GUI 的 Tauri a2a_* 命令全为 TODO stub（a2a/commands.rs:8-51），与 agents 页双轨并存（问题3） |
| acp 授权（策略表） | 无：acp-manage 页只管 workspace/状态/会话（views/acp-manage/），无 allow/deny（未找到） | 有：acp allow/deny/list（acp/mod.rs:29-35） | GUI 授权缺口 |
| tunnel 被访 serve | 无：Tauri 命令在（tunnel.rs:97-109；lib.rs:124）但前端 ipc.ts:266-270 无 serve 调用，remote-access 页仅展示状态（remote-access-view.tsx:29,35） | 有：tunnel serve（tunnel/mod.rs:13-14） | GUI 无法开被访侧（问题2） |
| tunnel 访问 | 有：remote-access 页（remote-access-view.tsx:74-84,155；generic-tunnel-card.tsx:93-101） | 有：tunnel connect（tunnel/mod.rs:16） | —— |
| authz 角色/绑定/default_role | 有：contacts 角色管理（views/contacts/role-manager-dialog 等）+ authz.rs:71-243 | 有：authz 域 role/bind/unbind/check/import（authz/mod.rs:20-48） | —— |
| repair | 无（未找到） | 无（cli.rs 命令域无 repair） | 仅独立 bin repair-helper（main.rs:38-44） |
| update | 有：views/update/ + update.rs:113-141（含下载安装） | 部分：update check/open（update/mod.rs:17-22），无下载安装 | —— |
| relay/rendezvous 服务端 | 无 | 无 | 仅旧 p2p-cli bootstrap（crates/p2p-cli/src/cli.rs:22-46） |

## 开放事实问题（不含结论）

1. lanOnly 前后端断裂：GUI 前端有开关（network-card.tsx:140-147）且 gui-contract.md:551 声称「PR4 已落 serde 双向兼容」，但 Tauri 后端 GuiConfig 无 lan_only 字段（types.rs:21-48）——前端保存该字段会被后端 serde 丢弃，GUI 节点启动也不消费（state.rs:293-307）；CLI 侧字段与消费齐备（cli types.rs:54-56；daemon.rs:28）。
2. tunnel 被访侧 GUI「命令在、界面无」：tunnel_serve_start/stop 已注册（tunnel.rs:97-109；lib.rs:124），前端 ipc.ts:266-270 只有 tunnelOpenDsh/tunnelOpen/tunnelStatus，全前端无调用（grep 未找到）；remote-access 页 serve 状态恒显默认关闭（remote-access-view.tsx:29）。
3. a2a GUI 双轨：Tauri a2a_list/publish/unpublish/allow/disallow 全是返回假数据的 TODO stub（a2a/commands.rs:8-51），同文件头注释却称「复用 acp-agent admin HTTP」；真实操作面在 agents 页直连 admin HTTP（agents-page.tsx:103-119）。同一能力两处入口，其一为空壳。
4. GUI 加好友不触发默认角色自动绑：auto_bind_default_role 唯一调用点在 CLI chat friend add（apps/cli/src/chat/friends.rs:166；实现 apps/cli/src/authz/default_role.rs:45）；GUI chat_friend_invite/chat_invite_accept（chat.rs:41,64）无对应调用，authzDefaultRole 字段 GUI 可读写（authz.rs:129-160）但 GUI 好友流程不消费。
5. llm-share 借出无显式关闸：serve 装配条件=offer live（serve/mod.rs:148-151），CLI offer 域只有 publish/show 无 unpublish（offer.rs:16-21），GUI offer publish 为覆盖式（backend.ts:25）；停止借出只能覆盖声明或停节点。
6. llm-share 闸1 保留回退双语义：LenderProxy 未注入 gate1 时回落内建 allowlist 判定（crates/llm-share-proxy/src/server.rs:66-67,197；tests authz_gate.rs:254-276 明示「无 gate1 时 allowlist 依旧权威」）；当前 GUI 装配已注入（proxy.rs:72），但库层允许装配方退回旧闸。
7. relay/rendezvous 服务端与主产品面脱节：生产装配点仅存旧调试 bin p2p-cli bootstrap（crates/p2p-cli/src/cli.rs:22-46；relay_serve.rs:68-71）；p2pctl/GUI 无部署与开关面；同时每个 GUI/daemon 节点默认内建 rendezvous 服务端受理注册（assembly.rs:82-84，rendezvous_public_only 默认 false 且两个 GuiConfig 均未暴露该字段）。
8. GuiConfig 类型双定义已漂移：GUI（gui types.rs:21）与 CLI（cli types.rs:35）各自维护一份契约镜像，字段集不一致（lanOnly 仅 CLI 有），注释自认「GUI 侧类型尚未含此字段」（cli types.rs:58）。
9. chat 入站无来源过滤：任意已认证 peer 的消息与媒体直接落库并广播事件（wire.rs:249-298，append_message :293、receive_media :265-272），非好友消息也会入库——chat.send 未接判定是已登记预留（authz-role-design.md:186），但「陌生人消息落库」这一现状本身未见登记。
10. acp-agent 生命周期悬空：GUI 经 ~/.dsh/acp/local-agent.json 读描述符（acp_descriptor.rs:2；agents-page.tsx:32 缺失即挂起并 console.warn），CLI/GUI 命令面均未找到拉起/守护 acp-agent 的入口（未找到）；--descriptor-disabled 存在防覆盖语义（cli.rs:62-63）暗示多实例风险。
11. tunnel 白名单双存储：GUI slot 白名单在 GUI 进程内存（tunnel.rs:63-68 add_allow），CLI serve 白名单在该进程启动参数（tunnel/serve.rs:51-60），两套互不相通、均不持久化。
12. repair-helper 数据根对齐靠人工：mint --data-dir 默认 ./p2p-data（main.rs:52,68），GUI authz 数据根=app 数据目录（authz.rs:5-6,66-68「CLI --data-dir 等价物」）；同机混用时绑定表是否同份取决于启动参数，无一致性校验（未找到）。
13. 底座 ConnectionGate 扩展点闲置：trait 与 set_gate 均在（crates/p2p-swarm/src/lib.rs:83；node.rs:82-84），生产 apps 零调用（grep 无命中），默认放行所有已认证 peer——权限收口时是现成挂载点但当前无消费者。
