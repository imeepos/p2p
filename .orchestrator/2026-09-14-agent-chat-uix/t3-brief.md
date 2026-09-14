# T3 任务书 · frontend · 会话区视觉对齐（定稿,派发 session 见账本）

## 目标
agent 会话区（chat-page / agent-conversation / composer / 空态）按 uix-spec.md 第 3 节差距清单做视觉对齐复刻。

## Consumes（先读,按序）
1. /Users/imeepos/ext512/p2p/.orchestrator/2026-09-14-agent-chat-uix/uix-spec.md 第 1、3 节
2. /Users/imeepos/ext512/p2p/.orchestrator/2026-09-14-agent-chat-uix/before-agent-chat-0914.png（before 截图,read_image 可读）

## 主控从 before 截图核实的丑点清单（按此对齐,逐项可判定）
a. assistant 回复 = 整块灰底面板,非对话气泡;用户/agent 消息无视觉区分层次
b. 「正常结束」stopReason 小字悬挂突兀
c. 输入区单行 input + 底部大片空白,与消息区割裂
d. 头像高饱和色块(绿/蓝/红)与灰白基调冲突;选中会话满宽高亮视觉噪
e. hairline(0.5px)缺失,区块边界生硬(spec token 表 #9)

## 实现边界（预置）
- 只动 views/chat/ 会话区视觉层与 acp/components/transcript.tsx / prompt-composer.tsx 样式；
  不改 transcript-model / 协议层；不做后端。
- 与 T2 并行但文件零交集（T2 = acp/components/session-sidebar + acp-store + i18n 侧栏键块；
  T3 = views/chat 视觉 + transcript/composer 样式类 + i18n 会话区键块）——i18n 键块按域分文件提交防冲突。

## 验收（预置）
- 差距清单逐项 before/after 截图证据（jsdom 渲染或 mock dev 页走查）；console 干净。
- 视觉回归不破坏既有 chat-render-matrix / chat-scroll 测试。

## 全局约束（硬性）
worktree 协议 + 收尾四步（分支 feat/uix-conversation,基于 origin/main）;函数 ≤60 行/文件 ≤300 行;
无 emoji;中文注释独立成行;与 T2（session-sidebar/acp-store/i18n 侧栏键块）文件零交集——
transcript.tsx/prompt-composer.tsx/chat 空态的 i18n 键走会话区键块,独立提交。
测试与实现同提交;不得破坏 chat-render-matrix/chat-scroll 既有断言语义。

## 预算与停止条件
实现 1 轮 + 修复 ≤2 轮。某丑点需要动协议/数据模型才可实现 → 该项标 BLOCKED 单列,其余照常交付。

## 汇报
session_link_send_parent 回报派发者（显式 id：session-e1cd6aa4-b444-4355-a8a5-cb3daf959e0d）,
状态枚举 + 分支名 + 测试退出码 + after 截图路径（对照 before 逐项标注）。
