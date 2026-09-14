# 计划 · agent 聊天页按钮反馈微波（v1）

> 来源：用户直接指令 2026-09-14「本机 agent 聊天页面 点击新建会话 大半天没有反馈，
> 多次连续点击后 进入聊天页面。所有按钮都需要优化：点击-微反馈loading-成功微提示-错误轻提示」。

## 根因链（git 证据）

- `1709ddd0`：ACP initialize 超时放宽到 120s（容忍 pump 退避 / dsh 冷启动）。
- `66c5a147`：session/new 挂 120s 慢速包络 + 超时包络回归测试。
- ⇒ newSession 最坏可阻塞 ~120s，期间按钮无 pending 态、不禁重复点击 → 用户连点。
- 现状：`apps/gui/src/components/feedback/toast.ts` 基建完备（toastSuccess/toastError 带去重、
  复制详情），但 agent 聊天页按钮普遍未接 pending/toast。

## 任务分解（并行：AF1 ⊥ AF2，文件零交集；共享契约 AsyncButton 已在主干）

> 修订 14:22：侦察发现 `components/feedback/async-button.tsx` 已存在（重入守卫/loading/
> 结果态/aria-busy），共享契约无需新建 —— 按并行手册「契约钉主干」改串行为并行。

### AF1（frontend）新建会话面反馈闭环
- acp-store：runNewSession 单飞（in-flight 重复调用返回同一 promise）+ `newSessionPending` 布尔。
- 接线：`views/chat/agent-conversation.tsx`（agent-new-session 按钮）+
  `acp/components/session-sidebar.tsx`（新建会话按钮）→ AsyncButton + toast。
- 失败 toast 单弹：先定位存量「新建会话失败」toast 源头（acp-flow-failures.test.tsx 有断言），
  复用其 key，禁止双弹。
- i18n：`chat.feedback.session*` 键块（zh/en/types 同步，独立小提交；先 grep 复用既有 key）。
- 独占文件：acp-store.ts、agent-conversation.tsx、session-sidebar.tsx（及其 test）、
  chat.feedback.session* 键块。

### AF2（frontend）agent 聊天页其余按钮反馈走查
- 消费主干既有 AsyncButton，走查 views/chat + acp/components 其余异步交互按钮逐个接线。
- 判据：异步（IPC/store promise）动作 → pending + 成功微提示/错误轻提示；
  纯本地同步 toggle（折叠/展开）不加 toast；已有完整反馈面的（composer 发送、copy-button）
  只审计补缺，不重造。
- 独占文件：AF1 清单之外的全部走查面；i18n 仅 `chat.feedback.actions.*` 子键块。
- 合并序：AF1 先并，AF2 rebase 后并（i18n 大文件按键块区域各自插入，冲突面可控）。

## 全局约束（逐字进任务书）
- 分支前缀 feat/acf-*；一个任务一个分支一个 worktree；子会话禁合并主干（合并由主控执行）。
- 单文件 ≤300 行（推荐 ≤200）、单函数 ≤60 行；禁止 emoji；注释零废话。
- i18n 键块注册类改动压成独立小提交。
- 测试红绿双向：先失败测试再实现。
- scope 争用：session-sidebar.tsx 为 uix 波规划文件，本波只加反馈行为不重构结构。

## 验收口径（波次级）
- 新建会话：点击即时 pending（disabled + spinner），期间重入被拒；成功 toast「会话已创建」；
  失败轻提示（错误 toast，不得与存量失败 toast 双弹）。
- 渲染矩阵测试覆盖 pending/success/error/重入四态；聚焦测试与 tsc/eslint 全绿退出码 0。
- console 零新增报错。

## 账本
- 进度：本目录 ledger.md；计划版本 v1（2026-09-14）。
