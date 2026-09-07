# UX-R2 修复波派单记录（2026-09-07）

维护人：协调会话（主会话）。各实现会话禁止修改本文档。
审计依据：docs/notes/2026-09-07-uxr2-audit-findings.md（26 findings：P0×2 / P1×8 / P2×16）。
与 R1（UX-E…K，已收官）为增量关系。基线：main @ b56c42f，工作区干净。

## 产品裁决（协调者拍板）

1. **R2-24（AcpView 死挂载）**：本轮不回挂 AcpView、不删死代码（能力对等需真机核验）。本轮只做最小可见性：chat agent 形态当存在待应答权限请求时给出可见指示+直达 contacts Agent 详情（复用现有 permission-panel 与 store 状态）。死代码取舍登记为债务，待真实流程终验后裁决。
2. **R2-13（llm-share 入口）**：批准升 rail，menu.def 属中央登记文件，必须压独立小提交。
3. **R2-19**：不改动组件；选做补一条引导失败横幅 e2e。

## 波 R2-F（并行 3 卡，文件所有权互斥）

| 卡 | 分支 | 覆盖 finding | 文件所有权 |
|---|---|---|---|
| R2F-A llm-share 域 | feat/uxr2a-llmshare | R2-01…R2-15、R2-13、R2-26 的 llm-share warn 部分 | views/llm-share/**、menu.def.ts（独立小提交）、rail 注册、相关 i18n 键 |
| R2F-B 监控族+全局 | feat/uxr2b-monitor-global | R2-16、R2-17、R2-18、R2-25、R2-26 的 acp console-watch 降级、R2-19(e2e 选做) | views/network/events/**、views/network/overview 趋势与最近事件卡、components/monitor/**、components/command-palette/**、console-watch 所在文件、相关 i18n 键 |
| R2F-C docs/更新卡/ACP | feat/uxr2c-docs-acp | R2-20、R2-21、R2-22、R2-23、R2-24（按上述裁决） | views/docs/**、设置更新卡 views/update 或 settings 内更新卡文件、ACP 失败文案源、views/contacts/agent-detail-drawer、相关 i18n 键 |

i18n 约定：各卡只增改本卡键区块，合并冲突在 feature 侧消化（AGENTS.md 规则）。
统一验收门禁（卡内自查 + 协调者主树机械复跑）：pnpm lint / typecheck / vitest run / build / check:i18n 全绿。
调试端口分配：A=5177/9237，B=5178/9238，C=5179/9239；收尾杀净 dev server 与调试 Chrome。

## 收官流程

各卡交付报告落 docs/notes/2026-09-07-uxr2{a,b,c}-delivery.md → 协调者机械验收（四门禁+gui-agent 抽查，不采信自报）→ ff-only 合并 → worktree/分支清理 → 真实流程终验（单独派卡）。
