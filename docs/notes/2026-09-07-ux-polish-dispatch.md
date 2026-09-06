# UX 打磨波次派单记录（2026-09-07）

维护人：协调会话（主会话）。各实现会话禁止修改本文档。
审计依据：docs/notes/ux-audit-20260907.md（28 findings：P0×5 / P1×12 / P2×11）。
与 9/5 轮（UI-A/B/C/D，已收官）为增量关系，不重复其范围。
与并行 LSG 波（llm-share GUI，3 worktrees）无文件交集；若碰撞以 AGENTS.md 冲突规则在 feature 侧消化。

## 基线（派单时实测）

main @ ec1b783：vitest 157 文件 919 用例全绿（冷启高负载下会出现超时假红，重跑即可）、lint 零告警、tsc 零错、check:i18n zh=en=998。

## 波1（并行 4 卡，文件所有权互斥）

| 卡 | 会话 | 分支 | 覆盖 finding | 文件所有权 |
|---|---|---|---|---|
| UX-E 关联选择器统一 | session-fbd26b10 | feat/uxe-entity-picker | F03/F11/F23/F25/F14/F24 | components/picker(新)、添加好友/添加Agent/拨号/群邀请选择四个弹窗、URL 契约消费端 |
| UX-F 表格人话化 | session-d919a87e | feat/uxf-table-human | F02/F12/F18/F19/F21 | 节点表/发现表/总览状态卡/消息邀请卡、共享 PeerId→名称 helper(新)、URL 契约生产端 |
| UX-G 危险确认与守卫 | session-8696f62c | feat/uxg-confirm-guard | F04/F05/F06 | 顶栏、停止节点弹窗、unsaved-guard 及中继接线、ACP 失败文案 |
| UX-H 聊天首公里 | session-18425f3f | feat/uxh-chat-firstmile | F01/F17/F07/F28 | 聊天页/会话列表/空态、composer 字数、chat stores 只读扩展 |

跨卡契约：#/network/peers?dial=<目标> 与 #/contacts?add=<peerId>，UX-E 实现消费端、UX-F 产生端。
验收：协调者在主树机械复跑四门禁（vitest/lint/tsc/check:i18n）+ gui-agent 页面抽查，不采信自报；通过后 ff-only 合并、worktree 清理。

## 波2（波1 合并后派，待定）

UX-I 事件页（F16/F20）、UX-J 地址表单标签与术语（F13/F10/F14-设置部分）、UX-K 操作路径与全局命名（F08/F09/F15/F22/F26/F27）。

## 观察与约定

- apps/gui/.env.development.local（VITE_MOCK_IPC=0）为某并行会话本机覆盖，mock 走查须显式 VITE_MOCK_IPC=1 独立实例（审计环境备注 1）。
- dev server 端口约定：并行会话错开使用 5176+，隔离 vite cacheDir；收尾必须杀 dev server 与 gui-agent 调试 Chrome。
- 归档工具注意：workspace_session_manage archiveSession 单会话调用会返回全工作区幂等归档清单，属预期回显，活跃会话不受影响。
