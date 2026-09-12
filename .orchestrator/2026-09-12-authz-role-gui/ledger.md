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
- 2026-09-12 23:19 TA 回报 DONE（三分支提交，头 e865ee8a，已 push origin）。
  主控亲核: diff 复核（update_role 显式失败分叉无半态；四命令契约逐字；
  tsv 四行带理由；§18.5 完整）+ worktree 亲跑 cargo test -p p2p-authz
  46 passed exit 0。TA 三点备注均合理（tsv 豁免理由按事实修正、update 无审计
  留后续轮、合并留主控）。完成了吗: TA 验收通过。下一步: 分支全量 make check
  （bash-291）绿后按收尾四步合并 TA；等 TB。
- 2026-09-12 23:19 Ruling: TA 备注「任务书原文称 role create 表外 key 报错附
  清单」系主控笔误，实际附清单的是 authz check 面——按事实落表，exempt 结论
  不变。依据: p2p-cli access.rs:38。错了的代价: 无（结论未变，仅理由修正）。
- 2026-09-12 23:20 等待窗口: main 推送 origin（172ecac1，pre-push 快门禁 PASS）；
  wtd-gui 残留实为非注册 worktree 的空目录（仅空 apps/ 子目录，git toplevel 解析
  回主仓），已删除。.worktrees 现仅 wra-backend / wra-frontend。
  在飞: TA 全量 make check（bash-291）、TB 前端卡。零迹象判据已具备（两 worktree
  均有提交/门禁活动迹象）。
- 2026-09-12 23:29 TB 回报 DONE（三分支提交，头 54a03133，已 push origin；vitest
  1431 passed / build / eslint / gui-dist-scan 全 exit 0）。主控亲核: diff 复核
  （ipc 契约形状与 TA 逐字对齐；入口 SettingsIcon 按钮 + RoleManagerDialog 条件
  挂载；store 三 action 失败 console.error + 上抛；PERM_LABEL_KEYS 九 key + 裸 key
  兜底；builtin 行无编辑删除钮）。TB 备注①（接口实名 IpcBackend 非 DshIpc）
  属实，按代码实际落契约，无碍。完成了吗: TB 验收通过。下一步: 等 TA 全量
  make check（bash-291）→ 合并 TA → TB rebase 后跑全量 make check → 合并 TB
  → 主树 check-fast 终检（ff-only 树同构，分支尖全量检查即主干检查）。
