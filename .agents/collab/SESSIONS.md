# 会话认领登记（append-only，先落 git 者得 scope）

> 规则：开工认领、完工释放；跨 scope 需 owner 书面同意并留痕；租约随波次刷新。

- [历史] 2026-09-11 dsh-tunnel 波（协调者 session-3373f897-f9c0-4a32-9af2-b9562eecc311）：九卡收官，main==origin/main @ a9ea63e1，过程债务清零（依据 .orchestrator/2026-09-11-dsh-tunnel/ledger.md 终态）。tunnel 域 scope 已释放。
- [认领] 2026-09-12 session-89ef72dc-c059-4ba7-91f3-89d38e6f48a8（dev-orchestrator）：认领 tunnel 泛化波（.orchestrator/2026-09-12-tunnel-any/）——scope：crates/p2p-tunnel、apps/cli tunnel 域、apps/gui tunnel 域（src-tauri/src/tunnel* + remote-access 页）、docs/protocol/specs/tunnel.md、docs/design/gui-contract.md §19、docs/ops/p2pctl-ai-guide.md tunnel 域。分支前缀 feat/wta|wtb|wtc|wtd|wte-。
- [释放] 2026-09-12 session-89ef72dc（dev-orchestrator）：tunnel 泛化波收官，
  六卡全并，main==origin/main @ 8d64663f（代码认证基 8f8e0daf，bash-203 全量
  绿），TE/TD 会话归档。tunnel 域 scope 释放。
- [认领] 2026-09-12 session-31ed5fb1-8ca8-4500-9ee6-1fa8a9c1c81d（dev-orchestrator）：认领
  authz 角色权限 GUI 化波（.orchestrator/2026-09-12-authz-role-gui/）——scope：
  crates/p2p-authz（update_role）、apps/gui/src-tauri authz 命令面、apps/gui authz 前端域
  （views/contacts 角色管理 + stores/authz-store + lib/ipc* + mock-authz + i18n
  contacts.authz.manager.* 键块）、scripts/check/cli-parity.tsv authz 段、
  docs/design/gui-contract.md §18。分支前缀 feat/wra-*。来源：用户直接指令。
- [释放] 2026-09-12 session-31ed5fb1（dev-orchestrator）：authz 角色权限 GUI 化波收官，
  TA/TB 六提交全并，main==origin/main @ 0d257fe3（TA 尖全量 make check exit 0 @
  ca9529cd 树、TB 尖全量 make check exit 0 @ 2653d429 树 = 合并后同构树，主干
  check-fast PASS），两卡归档，worktree/分支全清。authz 角色管理域 scope 释放。
  候选池：CLI authz role update 对等 + 审计事件、mock-authz.ts 拆分
  （.orchestrator/2026-09-12-authz-role-gui/ledger.md）。
- [认领] 2026-09-13 session-cd1cdce3-339d-4eb5-9414-d34cf87e51a1（dev-orchestrator）：认领
  权限统一收口 + 服务总控波（.orchestrator/2026-09-13-service-master/）——scope：
  crates/p2p-authz、crates/p2p（assembly/node 服务面）、apps/cli service/authz 域、
  apps/gui 服务开关面（settings + 服务卡片 + src-tauri 对应命令）、docs/design
  （authz-role-design / gui-contract 新章节）、docs/ops/p2pctl-ai-guide.md 对应域、
  scripts/check/cli-parity.tsv 对应段。分支前缀 feat/wsm-*。来源：用户直接指令。
- [认领] 2026-09-14 session-e1cd6aa4-b444-4355-a8a5-cb3daf959e0d（dev-orchestrator）：认领
  agent 聊天 UI 波（.orchestrator/2026-09-14-agent-chat-uix/）——scope：apps/gui/src/acp
  （session-sidebar 等会话组件 + acp-store 会话分组选择器）、apps/gui/src/views/chat/
  agent 会话面、docs/design/agent-chat-uix-spec.md（新增）。参考实现仅只读：
  /Users/imeepos/ext512/ymm-001/deepseek-harness/packages/client。分支前缀 feat/uix-*。
  禁触：权限/服务总控波 scope（settings 服务卡片、src-tauri、p2p-authz）、src-tauri
  （本波零后端改动）。来源：用户直接指令。
- [认领] 2026-09-14 session-e1cd6aa4-b444-4355-a8a5-cb3daf959e0d（dev-orchestrator）：追加认领
  tunnel GUI 操作面波（.orchestrator/2026-09-14-tunnel-gui/）——scope：apps/gui/src/views/
  remote-access/、相关 ipc-types/gui-contract §19 增量、docs/design/tunnel-gui-gap.md（新增）。
  该域前 owner（session-89ef72dc tunnel 泛化波）已释放，无争用。禁触：session-cd1cdce3 的
  settings/services-card 服务开关面、src-tauri/src/tunnel*（除非调研证明 IPC 缺口，先报我裁决）。
  分支前缀 feat/tgui-*。来源：用户直接指令。
- [接管] 2026-09-14 session-a8e83de1-d058-443a-82e9-21cbe2556d60（dev-orchestrator）：接管
  agent 聊天页按钮反馈微波（.orchestrator/2026-09-14-agent-chat-feedback/）——背景：用户直接指令
  （新建会话点击长时间无反馈，要求全按钮 点击-loading-成功微提示-错误轻提示）。接管依据：原 owner
  session-e1cd6aa4（agent 聊天 UI 波）账本停 2026-09-13 20:52、T2/T3 零派发、无 feat/uix-* 分支，
  主对主 talk 120s 超时无回复（claimToken 168dc843 已留档），按租约弃置接管，已提前声明。scope：
  apps/gui/src/components/feedback/（新增 use-async-action）、acp-store newSession pending 态、
  views/chat/agent-conversation.tsx 与 acp/components/session-sidebar.tsx 的按钮反馈面、
  i18n chat.feedback.* 键块；AF2 走查面 = views/chat + acp/components 其余交互组件。
  分支前缀 feat/acf-*。文件交集预警：session-sidebar.tsx 为 e1cd6aa4 波 T2（两级侧栏）规划文件，
  本波只加反馈行为不重构结构，其重写须以本波合并后的 main 为基。禁触：wsm 波（settings/服务开关面/
  src-tauri/p2p-authz）、tgui 波（remote-access）。若 e1cd6aa4 迟到回复主张该任务，以登记表先落 git
  为准协商回割。来源：用户直接指令。
- [释放] 2026-09-14 session-a8e83de1（dev-orchestrator）：agent 聊天页按钮反馈微波收官，
  AF1（新建会话反馈闭环，74276ae3）+ AF2（全按钮走查接线 + listWorkspaces 吞错契约修复，
  f75e0323）两卡全并，main==origin/main @ f75e0323，主干 check-fast + gui-check PASS
  （gui 242 文件/1492+ 测试绿），两子会话归档待办，worktree/分支全清。按钮反馈面 scope
  释放；移交 e1cd6aa4 域裁量项：lessons.md wsm-b3 重复两行（docs 装饰性）。
  候选池：见 .orchestrator/2026-09-14-agent-chat-feedback/ledger.md 收官节。
- [协调留痕] 2026-09-14 session-e1cd6aa4 ↔ session-a8e83de1：e1cd6aa4 迟到回复选 B，移交有效；
  其 T2（feat/uix-sidebar 两级树侧栏）已并 main@23c9a279，AF1/AF2 基线纠偏至该点。冲突序裁定：
  微反馈提交先进 main，e1cd6aa4 的「/chat 侧栏移植两级树」（feat/uix-conversation）rebase 对齐；
  移植时复用 AF1 的 AsyncButton + toast + newSessionPending 模式。e1cd6aa4 保留结构面，
  a8e83de1 保留按钮微反馈面（横切小改）。双方账本各自登记。

## 2026-09-14 agent 独立会话页（ACS 波）
- [认领] 2026-09-14 16:09 session-3aa89cd2（主控/lead）：scope=agent 会话独立页
  （apps/gui 新路由 /agent + /chat agent 形态拆除），波次
  .orchestrator/2026-09-14-agent-chat-standalone-page/；ACS1 派发中，
  分支前缀 feat/acs-*，当前 main==origin/main@2cb41b8a。用户三裁决入波次 plan.md。
- [认领] 2026-09-14 16:10 session-920a4593（frontend）：scope=agent 独立会话页（ACS1，立），
  分支 feat/acs-standalone / worktree .worktrees/acs-standalone，派发者 session-3aa89cd2。
  允许面 apps/gui/src（views/agent-chat 新建、acp、routes、config、i18n、App.tsx）+
  .orchestrator/本波 + 本文件；禁触 /chat 页内 agent 形态与 contacts/acp-manage 深链源头
  （ACS2 专属）、src-tauri/crates/docs/design。
- [认领] 2026-09-14 session-b2d05d3f（frontend）：scope=/chat agent 形态拆除（ACS2，破），
  分支 feat/acs-chat-removal / worktree .worktrees/acs-chat-removal，派发者 session-3aa89cd2。
  基线 origin/main@da861ce1；允许面 apps/gui/src（views/chat 拆除主体、contacts/acp-manage
  深链源头、i18n 死键）+ .orchestrator/本波 + 本文件；禁触 /agent 新页内部行为、
  routes/agent-redirect.ts 兜底、src-tauri/crates/docs/design。收尾仅①push，②③④主控执行。
- [完工释放] 2026-09-14 18:54 session-3aa89cd2（主控/lead）：ACS 波收官——ACS1
  （session-920a4593）+ ACS2（session-b2d05d3f）全并主干，rebase 后
  main==origin/main@68246e76，主干 check-fast exit=0；ACS3 交叉评审
  （MiniMax-M3 subagent）APPROVE 零 Critical；worktree/分支本地远端全清，
  两子会话归档。scope=agent 会话独立页 释放。候选池与收官报告见
  .orchestrator/2026-09-14-agent-chat-standalone-page/plan.md 收官节。

## 2026-09-16 FTP over P2P（goal-9f609d90）
- [认领] 2026-09-16 03:02 goal-9f609d90（FTP 目标会话）：scope=在 p2p 底座实现 FTP 协议
  （新建 crates/p2p-ftp + docs/protocol/registry.toml 与 wire-protocol.md §3.2 登记 +
  specs/ftp.md + README crate map 行），分支 feat/ftp-protocol / worktree
  .worktrees/ftp-protocol，任务来源=用户目标「在p2p协议基础上实现ftp协议」。
  禁触 .worktrees/rd-wire、.worktrees/wsm-b2 与他波在飞面（apps/gui、p2p-service 注册表闭集）。
- [认领] 2026-09-15 session-rd-goal（goal round 主会话）：认领远程桌面控制波
  （.orchestrator/2026-09-15-remote-desktop/）——scope：新建 crates/rd-wire（协议编解码，
  本轮）、后续 crates/rd-capture|rd-input|rd-clipboard|rd-fs|rd-service、apps/gui
  views/remote-desktop 域、src-tauri rd 命令面、docs/design/remote-desktop-plan.md（新增）、
  docs/protocol（registry.toml + specs/rd-*.md + wire-protocol.md §3.2 对应行）、
  docs/ops/p2pctl-ai-guide.md rd 域（后续）。分支前缀 feat/rd-*。来源：用户直接指令
  （远程桌面控制，要求覆盖 rustdesk 核心功能至商用程度）。
  禁触：wsm 服务总控波（feat/wsm-b2，session-cd1cdce3 在飞，settings/services-card、
  src-tauri 服务开关面、p2p-service）、tunnel 域（已释放但 GUI remote-access 属 tgui 波
  已收官，只读复用）。依赖注意：远程桌面服务开关接 p2p-service 注册表，wsm 波合入后接线。
 (docs(collab): 远程桌面波 scope 认领登记)
- [完工释放] 2026-09-16 05:06 goal-9f609d90（FTP 目标会话）：FTP over P2P 收官——
  crates/p2p-ftp 全链落地（/ftp/ctrl/1 + /ftp/data/1 登记 23 号注册表），
  15 单测 + 4 双节点 E2E 绿，全量 make check 绿（test 段一次 SIGKILL 重跑过），
  rebase 解 rd-wire 撞 registry/wire-protocol 尾部冲突（双方登记块都保留），
  main==origin/main@68abe761，worktree/分支本地远端全清。scope=FTP over P2P 释放。
  后续候选：p2pctl ftp 子命令、serve.ftp 服务开关闭集登记（需改设计表，先问）、authz 收编。

## 2026-09-16 FTP 收尾波（goal-9f609d90 后续）
- [认领] 2026-09-16 08:18 goal-9f609d90（FTP 目标会话续）：scope=FTP 收尾波
  FT1-FT6（crates/p2p-ftp 语义完善 + apps/cli ftp 域 + p2p-service/p2p-authz
  闭集扩容），计划 .orchestrator/2026-09-16-ftp-closeout/plan.md，分支
  feat/ftp-closeout / worktree .worktrees/ftp-closeout。禁触 rd-m2、wsm-b2 在飞面。
- [完工释放] 2026-09-16 10:58 goal-9f609d90（FTP 目标会话续）：FTP 收尾波 FT1-FT6
  全并主干（main==origin/main@2db444e1），全量 make check 绿。FT1 死变体清理、
  FT2 HiddenStores（STOR 失败零残留）、FT3 LIST 条目上限（552）、FT4 p2pctl ftp
  七命令（真机冒烟 pwd/put/ls/mkdir/get/delete/rmdir + StaticAuth 530 路径）、
  FT5 serve.ftp 闭集第 11 项 + daemon 装配（开关 AND ftp.json 双条件）、
  FT6 file.read/file.write 权限 key + Authorizer 接缝 + authz:true 桥。
  rebase 解 vdrive 撞尾冲突（双方保留），闭集锚点四处波及面（cli/tauri/
  gui mock/gui-contract）全同步。worktree/分支本地远端全清。scope 释放。

- [开工认领] 2026-09-17 session-72b40bd2（config-centralization 主控）：配置集中化波
  W1（CC1 前端设置页远程访问区 + CC2 GuiConfig 扩展与装配消费），分支 feat/cc-front、
  feat/cc-rust，scope=apps/gui/src、apps/gui/src-tauri、apps/cli、gui-contract §3；
  与 feat/wsm-b2（service master）并存，CC 不触 services/authz 域。
  任务来源：用户指令「配置都放配置页面，立即安排，并行，子会话轻量自验」。

- [完工释放] 2026-09-17 session-72b40bd2（config-centralization 主控）：W1 全并主干
  （main==origin/main@976d502f），统一门禁 check-fast + gui-tauri-check 全绿。
  CC1（feat/cc-front 59aca08e，设置页远程访问区三配置 + rd 卡读默认 + i18n 1793 对齐）
  与 CC2（feat/cc-rust 7233c2be，GuiConfig 三字段双镜像 + rd/tunnel 装配消费 +
  白名单持久化 + rd-host --fps 死参数修复 + gui-contract §3 同步）随过随合。
  worktree/分支本地远端全清，scope 释放。W2/W3 触发器与 FTP/static-peers 决策备忘录
  见 .orchestrator/2026-09-17-config-centralization/w2-decision-memo.md。

- [完工释放] 2026-09-17 session-72b40bd2（config-centralization 主控）：W2a 收官——CC3（feat/cc3-front 361d803b）设置页新增 rendezvous/中继/默认角色三编辑入口（零新命令，复用 9 组既有键），check-fast 全绿。worktree/分支全清。W2b（FTP/static-peers）等用户对 w2-decision-memo.md 的 A/B 选择。

- [完工释放] 2026-09-17 session-72b40bd2（config-centralization 主控）：W2b 收官——CC4（feat/cc4-rust 30d9c1e4，5 命令 + 静态对端装配接线补料，cr ates/p2p 可见性放开影响面声明，CLI daemon build_node 接线置于 lan_only 早退前）+ CC5（feat/cc5-front 8211d8d3，FTP 卡 + 静态对端卡 + i18n 1842 对齐，顺手修 AddressListEditor 行级校验隐性 bug），全并主干。统一门禁全绿。worktree/分支本地远端全清。W3（ACP token 明文迁移）触发器就绪。
