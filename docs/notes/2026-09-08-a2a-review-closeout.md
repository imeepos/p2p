# A2A over P2P 方案四路审查结论（2026-09-08）

审查对象: docs/design/a2a-over-p2p-design.md v0（344 行）。四路并行 subagent 审查，
全部裁决 REVISE，共 56 条发现（架构 20 / 安全 10 / GUI 12 / 范围 14），均已合并进 v1 定稿。
本文件为审查痕迹留档（append-only），逐条映射到 v1 修订动作。

## 架构审查（46f084aa）：20 条，major 10 + minor/nit 10
- F1 → v1 §5：task 相改 JSON-RPC 2.0 帧形（id 关联 + error 帧 + 应答帧 + subscribe ack）
- F2 → v1 §5.2/Q10：1 task = 1 流；cancel = 该流子进程 quiesce + status cancelled + 审计
- F3 → v1 §2/§3/Q11：task⇄ACP 桥 = acp-agent 内嵌 ACP client；v1 只发 TextPart，FilePart 接收渲染、发送显式拒绝
- F4 → v1 §7.4：下架改独立 card/remove 帧（弃 ttlSecs=0 墓碑，过不了验签时间窗）
- F5 → v1 §7.1：发现 TTL 与订阅簿寿命解耦——心跳 push 刷新（TTL/2），2×TTL 无心跳才除名
- F6 → v1 §5.1：card/list 与 push 按请求方 PeerId 过滤可见性；无公开卡的节点不注册进命名空间
- F7 → v1 §7.3/A2A5：邀请帧 wire 对齐 /im/invite/1 纪律（nonce/expiry/invitee 绑定/回执签名）
- F8 → v1 §9：权限路由矩阵写死（public：think=桥代答，read/fetch/execute/edit/delete=ask OwnerLocal，绝不 RemoteGui）
- F9 → v1 §3/A2A3/A2A4：console 协议感知扩展（?proto 加法参数）归 A2A3；WS 契约变更同步 gui-contract.md
- F10 → v1 §5.2：流式 message 帧带 messageId + 客户端去重 + task/get 快照合并规则
- F11 → v1 §7.4：卡片事件走 console 独立事件通道（acp-console），node-event 判别联合不动
- F12 → v1 §3/A2A1：协议 ID 自 crate 定义（crates/a2a::PROTOCOL_ID）+ wire-protocol §3.2 表登记
- F13 → v1 §4.3：AgentBook 版本替换（version 升序）+ 重复 subscribe 幂等
- F14 → v1 §12：公开池规模上限 512 认知写入风险
- F15 → v1 §3：crates/a2a 依赖仅 serde/serde_json/bs58/ed25519（零网络零进程，非零依赖）
- F16 → v1 §5.2：负空间声明（thoughts/tools/input_required/rejected 在 v1 的呈现与拒绝）
- F17 → v1 §4.1：agentId 限 [a-z0-9-]，禁非 ASCII
- F18 → v1 §5：帧 v 字段仅 card 相首帧带，task 相 JSON-RPC 版本由 jsonrpc 字段承载
- F19 → v1 §11：A2A2 拆分 + 邀请链验收逐步化 + itest stub 化
- F20 → v1 §6：私有授权撤销传播（card/remove + push 到已同意 peer）

## 安全审查（1d660079）：3 blocker + 4 major + 3 minor
- F1(b) → v1 §9：公开 agent 强制 sandbox scope；read/fetch 降 ask（不静态放行）；think 桥代答
- F2(b) → v1 §9：公开 agent ask 一律 OwnerLocal（本地审计 + reject-once 占位），绝不 RemoteGui
- F3(b) → v1 §7.3：邀请=签名凭证帧（nonce 一次性 + 过期 + invitee 绑定 + 回执签名），对齐 social-discovery-plan §P2
- F4(m) → v1 §5.1：card 相按请求方过滤，private 仅授权清单
- F5(m) → v1 §4.3：AgentBook 钳制（TTL≤3600、issued_at 偏差≤300s、容量上限、拨号令牌桶）
- F6(m) → v1 §5.2：每 peer 独立子进程（复用 slot 模型），task 操作按创建者 PeerId 鉴权
- F7(m) → v1 §9：宿主级总量限流 + FilePart≤4MiB + 每 task 输入累计上限 + 配额审计
- F8-F10(min) → v1 §4/§7/§13：字段一致性 deny_unknown_fields、私有卡保密边界、wire-protocol 同步

## GUI 一致性审查（b598c8d2）：4 major + 8 minor/nit
- F1(m) → v1 §8.3：新增独立 query 键 ?a2a=（不塞 ?agent=，避免撞 ACP 焦点）
- F2(m) → v1 §8.3/Q5：A2A 记录区=新 A2aConversation 复用 message-list/message-bubble（消息形态天然适配 parts），与 ACP transcript 栈分家
- F3(m) → v1 §11 A2A3：验收路径改 src/views/agents
- F4(m) → v1 §8.3/A2A4：ConversationKind 触碰面全列（conversationKey/kindMark/selectEntry/EmptyState/unread）+ conversation-entry 拆 conversation-entry-a2a.ts
- F5(min) → v1 §8.4：私密邀请 in 向 pending 并入 selectPendingInviteBadgeCount（非 use-unread-total）
- F6(min) → v1 §7.4：卡片事件归一 console WS 通道，node-event 判别联合不动
- F7(min) → v1 §8.2：PresenceTone 扩 gray（对应 StatusBadge neutral）；圆角按既有 token（rounded-lg）
- F8(min) → v1 §8.2/A2A3：MessageSectionHeader 泛化为共享组件（新增 tone 扩展，避免双源）
- F9(nit) → v1 §8.1：/agents rail 位置=llm-share 后 settings 前；图标 Sparkles；Cmd/Ctrl+8；顺带修正 App.tsx 过期注释
- F10(nit) → v1 §3：proto_ids 路径写全 crates/p2p-relay/src/lib.rs
- F11(min) → v1 §8.2/Q8：私有邀请同意免备注输入（agent 邀请按 agent 事务处理）
- F12(nit) → v1 §8.2/Q9：skills 输入=chip 输入（新建小组件，≤10 条，trim+去重）

## 范围与可行性审查（d07fa66d）：6 major + 8 minor/nit
- F1(m) → v1 §11：A2A2 拆 A2A2a(card)/A2A2b(task)，验收拆 card 链/task 链
- F2(m) → v1 §11：验收显式含 acp-agent 测试域（make check 不覆盖独立 cargo 项目）
- F3(m) → v1 §11 A2A5：双 GUI itest 降级为 agent 级 itest + GUI 组件测试 + 单节点 e2e 脚本
- F4(m) → v1 §4.2/A2A1：签名信封提炼 p2p-identity::signed 泛型 Signed<T>（llm-share-offer 内部委托，wire 零变更）
- F5(m) → v1 §11 A2A3：验收路径 src/views/agents
- F6(m) → v1 §11：每阶段补 LOC/文件数/耗时估计
- F7(min) → v1 §3/A2A1：wire-protocol §3.2 表加 /a2a/1 行
- F8(min) → v1 §11 A2A5：cli-parity.tsv 加行 + p2pctl-ai-guide.md 同步
- F9(min) → v1 §8.1：中央登记补 App.tsx
- F10(min) → v1 §7.4：事件契约权威文件=apps/gui/src/lib/ipc-types.ts NodeEventJson
- F11(nit) → v1 §11：itest 默认 acp-echo-stub，真 dsh #[ignore]+SKIP 信号
- F12(nit) → v1 §11：mDNS 不进 A2A2 验收（单测覆盖即可）
- F13(nit) → v1 §8.3/A2A4：chat-page.tsx 逼近 300 行，A2aConversation 独立组件
- F14(nit) → v1 §14：Q1 真 A2A；Q2/Q3/Q4 同意推荐项

## 纪律冲突面（合并后）
1. acp-local-loop 分支（17 文件 +629，apps/acp-agent 全域）与 A2A2 文件域重叠 → A2A2 动工前先反向同步 main 确认其合入。
2. ux3-diag 分支改 i18n locales → A2A3 前反向同步，i18n 登记独立小提交。
3. 短命分支：A2A1 完成即合并 main，不攒 mega 分支。
4. cli-parity/ai-docs-sync 登记随 A2A5 独立小提交。
5. crates/a2a 文件级拆分防 line-limit 红线。

