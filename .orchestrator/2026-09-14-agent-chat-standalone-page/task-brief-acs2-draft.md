# ACS2 任务书草案：聊天页拆除 agent 形态（破）

> 状态：DRAFT —— ACS1 合并主干后定稿再派发；届时以主干实际形态
> （/agent 页最终结构、redirects 落点）校对本草案，禁止盲发。
> 类型：frontend ｜ 分支 feat/acs-chat-removal ｜ 依赖：ACS1 已并 main。

## 目标（一句话）

agent 会话唯一入口收敛到 `/agent`：/chat 移除 agent 形态与会话列表条目，
contacts 与 acp-manage 深链源头改指新页，i18n 死键清理。

## 预置事实（2026-09-14 主控已核，ACS1 合并后复核）

- 拆除面：chat-page.tsx（AgentConversation 挂载、?agent=/?kind=agent 分支）、
  use-conversation-entries（agent 条目产出）、chat-page 测试矩阵。
- 深链源头（redirects 已兜底，此处改源头）：
  views/contacts/{detail-agent,agent-section,agent-detail-drawer,endpoint-add-dialog}.tsx、
  views/acp-manage/sessions-card.tsx → 改指 /agent?endpoint=...。
- agent-conversation.tsx 及 chat.agentPane.* 键：拆除后若无引用随之退役；
  agent-conversation-inject.ts 的 VITE_MOCK_IPC 注入入口随迁移走（确认新页已带）。
- redirects.tsx 中 `/chat?agent=` 中转是否保留：以 ACS1 实现为准
  （源头改完后可留一层兼容，写测试锁行为即可）。

## 验收标准（草案，定稿时机械核验口径不变）

1. /chat 页与 use-conversation-entries 无 agent 分支；聊天页渲染矩阵测试更新后绿。
2. contacts 四文件 + acp-manage/sessions-card 深链指向 /agent?endpoint=，grep
   `chat?agent=` 源头零命中（redirects.tsx 兼容层除外）。
3. i18n 死键清理：`grep -r "chat.agentPane" apps/gui/src` 仅剩 locale 文件或零命中，
   删除的键同步 types。
4. 聚焦测试 + typecheck + lint 退出码 0；console 零新增报错；check-fast + gui-check 绿。

## 边界

- 不动 /agent 新页内部（ACS1 产物，除接线必需的 minimal 改动需在汇报中单列）。
- 不做新功能；纯迁移+拆除；单提交一个可独立 revert 的变更块。
- 预算 ≤2 修复轮次；停止条件：新页行为与草案假设不符需回改 ACS1 产物 → BLOCKED 上报。
