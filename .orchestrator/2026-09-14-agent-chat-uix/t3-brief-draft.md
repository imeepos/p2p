# T3 任务书草稿 · frontend · 会话区视觉对齐（待 T1 产出后定稿派发）

## 目标
agent 会话区（chat-page / agent-conversation / composer / 空态）按 uix-spec.md 第 3 节差距清单做视觉对齐复刻。

## Consumes
uix-spec.md 第 1、3 节。

## 实现边界（预置）
- 只动 views/chat/ 会话区视觉层与 acp/components/transcript.tsx / prompt-composer.tsx 样式；
  不改 transcript-model / 协议层；不做后端。
- 与 T2 并行但文件零交集（T2 = acp/components/session-sidebar + acp-store + i18n 侧栏键块；
  T3 = views/chat 视觉 + transcript/composer 样式类 + i18n 会话区键块）——i18n 键块按域分文件提交防冲突。

## 验收（预置）
- 差距清单逐项 before/after 截图证据（jsdom 渲染或 mock dev 页走查）；console 干净。
- 视觉回归不破坏既有 chat-render-matrix / chat-scroll 测试。
