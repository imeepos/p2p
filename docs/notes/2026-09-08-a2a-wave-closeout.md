# A2A over P2P 波收口记录（A2A1-A2A5）

> 日期: 2026-09-08 | 协调: session-66d1052169a0 | 设计真值源: docs/design/a2a-over-p2p-design.md v1 定稿
> 账本事实：原 72 卡账本（A2A1-A2A6 在列）已被 inline-acp-pump 波重构为 7 卡新结构，
> A2A done 记录以本文件为准。

## 合并轨迹（main 前进链）
- A2A1 crates/a2a 纯库 + p2p-identity::signed → 63f5b2c 起（A2A2a 前合入）
- A2A2a card 相（AgentStore/handler/admin/在场发布）→ eacc82b
- A2A2b task 相（JSON-RPC 流循环/桥/门禁/限流/权限维度）→ 08946e0
- A2A3 GUI /agents 页 + console ?proto + 卡片事件通道 → b13759a
- A2A4 GUI A2A 聊天（?a2a=/A2aConversation/断线恢复）→ 6e2a564
- A2A5 私有邀请全链 + p2pctl a2a → 287c812
- 各阶段验收命令真实跑绿（A2A2b/A2A3/A2A4/A2A5 协调者独立复跑，不信自报）

## 显式降级登记（后续波次清单）
1. A2A3: 「编辑」仅 visibility/enabled 可改（宿主 admin PUT v1 数据面）
2. A2A4: A2aMessage.receivedAtMs 由 store effect 填充（渲染期禁 Date.now，a2a/online.ts 注释）；use-conversation-entries 的 tsMs 取 receivedAtMs ?? 0
3. A2A5: GUI「分享」动作的 admin HTTP 邀请生成端点已补齐（收口波 POST /a2a/agents/{id}/invite）；i18n 键路径已确认完整（contacts.agents.share.* + agents.* 双命名空间）
4. 全域: 真 dsh #[ignore] itest 与 SKIP 信号 = A2A6 范围，未做

## A2A6 派发前置条件（重要）
inline-acp-pump 波（refactor/inline-acp-pump，账本 INLINE-T1..T7）正在删除 apps/acp-console
并把库面迁入 crates/acp-pump——A2A3/A2A4 的 GUI↔console 通道承载（§17）与 acp-console 内
a2a 测试将被该波重构。**A2A6（契约收口/E2E 稳定化）必须等该波合并后按新形态派发**，否则返工。

## 教训（self-evolving 喂回要点）
- 子代理"验收绿"必须协调者独立复跑（A2A4 两轮：lint 管道假 GREEN + gui 硬编码 CJK 唯一红）
- 大文档编辑禁整本重写（A2A5 把 1625 行指南写成 265 行毁 82 条目）——append-only 对文档同样适用
- 管道 `cmd | tail` 会吃掉退出码，验收判据必须直读 RC（A2A5 "ACC-GREEN" 假信号实证）
- 子代理额度死亡=换 provider 重新派单，WIP 在 worktree 天然持久，接续前先查产物
- 共享账本会被并行波重构——波次收口记录落 docs/notes/ 才是持久真相源
