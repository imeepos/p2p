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
