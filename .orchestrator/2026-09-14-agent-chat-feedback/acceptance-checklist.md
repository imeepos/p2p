# 验收门核对单 · AF1/AF2（主控验收用，机械核验）

> 依据：task-spec-frontend 验收门 + TODO.md 微波次验收标准。子会话 DONE ≠ 验收通过；
> 逐条要行为证据，口头成功不作数。

## AF1（feat/acf-af1-session-feedback）

- [ ] 基线核验：分支包含 23c9a279（`git merge-base HEAD origin/main` = 23c9a279 或其后代）；
       session-sidebar.tsx 为两级树版本且结构零重构（diff 只含按钮反馈相关行）。
- [ ] 红绿证据：单飞/pending 测试先红后绿的陈述可信（抽查测试文件：注释或提交序里有失败态；
       至少核对「连点只触发一次 store 动作」用例存在且断言的是调用计数而非 mock 假象）。
- [ ] 两入口接线：agent-conversation.tsx（agent-new-session）与新侧栏新建按钮均为 AsyncButton
       （grep 确认无残留 `void newSession()` 裸 Button）。
- [ ] 单弹：错误 toast 只弹一次——定位存量失败 toast 源头后的二选一处置有测试断言（计数=1）。
- [ ] 存量测试保绿：acp-flow-failures / acp-view-interactions / session-sidebar.test / 23c9a279
       新增的两级树测试全绿。
- [ ] i18n：chat.feedback.session* 在 zh/en/types.ts 三处齐；键块为独立提交。
- [ ] 自验退出码：聚焦 vitest / typecheck / lint 三个 0（贴输出）。
- [ ] 行数红线：改动文件 ≤300 行（make line-limit 或 wc 抽查）。

## AF2（feat/acf-af2-button-walkthrough）

- [ ] 走查清单完整：views/chat + acp/components（除 AF1 独占）+ conversation-row/context-menu，
       每控件一行裁决（接线|无需|已达标），零遗漏。
- [ ] 接线批：每个「接线」按钮 pending/成功/失败三态齐，错误 toast 带 context；
       新增测试断言 pending 可见 + 失败 toast 文案。
- [ ] 防洪水：同步 toggle 无 toast；已达标面（composer 发送、copy-button、确认弹框）未被重造。
- [ ] AF1 独占文件零触碰（agent-conversation / session-sidebar / acp-store / chat.feedback.session*）。
- [ ] i18n 仅 chat.feedback.actions.* 子键块；三处同步；独立提交。
- [ ] 自验退出码三个 0；console 零新增报错证据。

## 主干汇总（两卡合并后）

- [ ] 合并序：AF1 先 push + ff-only；AF2 rebase origin/main 解 i18n 冲突（feature 侧消化）后再并。
- [ ] 主干实跑：`make check-fast` 或按域裁剪门禁（gui-check）真实退出码 0。
- [ ] 抽查：主干 grep 裸 `void newSession()` 应零命中（chat 面）。
- [ ] 账本闭环 + 收尾四步（push/ff-only/worktree remove/branch -d + 远端删）。
