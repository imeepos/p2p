# 进度账本：2026-09-12-authz-role-gui

> 双层账本。Facts = 已核实事实；Guesses = 假设（必带过期条件）。
> 主控: session-31ed5fb1-8ca8-4500-9ee6-1fa8a9c1c81d

## 任务账本

| # | 卡 | 类型 | 分支 | 状态 | 派发时刻 | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | TA backend | backend(rust) | feat/wra-backend | dispatched | 2026-09-12 22:54 | session-b2362705-31ea-41f7-aa98-2a26d6d4c35f |
| 2 | TB frontend | frontend | feat/wra-frontend | dispatched | 2026-09-12 22:54 | session-086bbd72-104c-4569-a89b-fdcc725659df |

Facts:
- main == origin/main @ 75e98ebf（开工时已 fetch 核对，树干净）。
- 缺口核实: §18 契约明确 GUI 本轮无角色创建入口；p2p-authz 无 update_role；
  src-tauri 无 role create/update/delete/permissions_list 命令。
- 依赖结构: TB 消费契约文本（已逐字钉进两份任务书），文件零交集 → 并行合法；
  合并序 TA→TB。
- cli-parity 门禁要求新 GUI 命令逐行登记 tsv（authz 段 76-82 行先例在案）。
- .worktrees/wtd-gui 为上波残留（干净、无独立分支），本波收官顺手清理。

Guesses:
- [Guess] src-tauri authz.rs 加四命令后仍 <300 行，无需拆模块。过期条件: TA 回报
  行数门禁红或自行拆分。
- [Guess] friend-role-dialog.tsx 256 行加入口按钮后 <300。过期条件: TB BLOCKED
  报行数超限。

计划版本: v1（派发即初版，无变更）。

## 进度账本（每事件一行，五问裁决）

- 2026-09-12 22:42 用户直派: GUI 无法指定角色权限，要求修复。完成了吗: 开工。
- 2026-09-12 22:49 事实源对齐完成（authz-role-design §4/§5/§10、契约 §18、
  p2p-authz ops、src-tauri authz.rs、cli-parity 门禁、mock-authz 先例、self-evolving
  references 相关条目）。有进展吗: 计划就绪。
- 2026-09-12 22:54 plan v1 + 两份任务书落盘，SESSIONS.md 认领登记，提交存档，
  双 worktree 建立，两卡并行派发（session_link_create greeting=任务书全文）。
  下一步: 结束回合等回报；等待窗口做 wtd-gui 残留核查收尾准备。
