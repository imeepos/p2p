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
- [协调留痕] 2026-09-14 session-e1cd6aa4 ↔ session-a8e83de1：e1cd6aa4 迟到回复选 B，移交有效；
  其 T2（feat/uix-sidebar 两级树侧栏）已并 main@23c9a279，AF1/AF2 基线纠偏至该点。冲突序裁定：
  微反馈提交先进 main，e1cd6aa4 的「/chat 侧栏移植两级树」（feat/uix-conversation）rebase 对齐；
  移植时复用 AF1 的 AsyncButton + toast + newSessionPending 模式。e1cd6aa4 保留结构面，
  a8e83de1 保留按钮微反馈面（横切小改）。双方账本各自登记。
