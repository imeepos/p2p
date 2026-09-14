# 本仓 ACP vs t3code effect-acp 缺陷与不足分析（AC1 · 只读对比评审）

- 日期: 2026-09-14 ｜ 评审人: AC1 子会话（只读，未改任何仓库文件）
- 本仓组件: apps/acp-agent、crates/acp-pump、apps/acp-common、apps/gui/src/acp、apps/cli/src/acp
- 对标: /Volumes/sker/resources/coding/t3code/packages/effect-acp（schema 生成自 ACP spec v0.11.3，meta.gen.ts:2）
- 协议基准: docs/design/acp-over-p2p-design.md（下称「设计 §」）+ 对标侧从官方 spec 生成的 schema 面
- 方法: 窄路径精读两侧源码与测试，逐条双证（本仓 file:line / 对标 file:line）。只报本仓缺陷；对标侧更优写法仅作证据。

## 结论速览

本仓的独有价值（续连窗口、权限瀑布、mcp 监狱、字节泵护栏）effect-acp 完全没有，不必自卑；
但「协议层正确性机制」全面落后：无 schema 单一来源、无运行时校验、无类型化错误、方法面已现漂移
（session/delete 悬案）、elicitation/terminal/load/fork 能力缺失、契约测试无 spec 基准。
共 14 条发现：Important 9 / Minor 5 / Critical 0。

---

## 发现清单

### 维度 1：协议层实现质量

**F1【Important】GUI 协议面零运行时校验，catch-all 类型静默吞变形数据**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:192-212（`JSON.parse as JsonRpcWire` 后仅按 method/id 鸭子类型分发）；apps/gui/src/acp/protocol.ts:156-168（`SessionUpdate` 联合类型末位 catch-all `{ sessionUpdate: string; [extra: string]: unknown }`）
- 对标证据: t3code src/protocol.ts:74-77、264-303（`decodeSessionUpdate`/`decodeElicitationComplete` 逐通知 Schema 解码，失败产出带 operation/method 的 `AcpProtocolParseError`）；src/errors.ts:31-77（issueCount/issueKinds/maximumPathDepth 结构化诊断）
- 影响: 上游 agent 字段变形或版本升级时，GUI 不报错、不计数、照常渲染错数据；违背「失败路径必须留可观测信号」以协议层标准衡量。
- 借鉴建议: **引入**。对 session/update、request_permission、config_option 等语义面加轻量 schema（zod）解码 + 失败计数，不必全量生成。

**F2【Important】方法面手写漂移：session/delete 在官方 spec 全集中不存在**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:310-313（`sessionDelete` 发 `"session/delete"`，注释自述「真机对拍后改正」但仍在）；apps/gui/src/acp/mock-acp-ws.ts:148-163（mock 自造方法面）
- 对标证据: t3code src/_generated/meta.gen.ts:8-18（v0.11.3 方法全集：session/close 存在，无 session/delete）；src/rpc.ts:55-59
- 影响: 对任何 spec 兼容 agent，「删除会话」必然 -32601 methodNotFound；方法是字符串字面量散落各处，无编译期保护。
- 借鉴建议: **引入**。先改 delete→close，再把方法名收拢为单一常量模块（见 F13）。

**F3【Important】错误建模：字符串化 Error vs 类型化错误闭集**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:220-224（`reject(new Error(JSON.stringify(msg.error)))`，错误码无类型层）；apps/acp-common/src/error.rs:6-40（ErrorCode 闭集只覆盖握手/护栏/分享，不含 JSON-RPC 语义面）
- 对标证据: t3code src/errors.ts:185-364（AcpRequestError 携带 code/message/data + 标准码工厂 -32700/-32600/-32601/-32602/-32603/-32000/-32002）；src/errors.ts:366-375（AcpError 判别联合）
- 影响: GUI 错误分支只能字符串匹配或二次 JSON.parse，error-help 分类不可靠；新增错误语义靠散点改动。
- 借鉴建议: **引入**。GUI 侧建立 JSON-RPC 错误码闭集类型 + 结构化字段。

**F4【Minor】malformed 行无 -32700 应答、未知带 id 方法无 -32601 应答**
- 本仓证据: apps/acp-agent/src/mcp.rs:29-31（非 JSON 行 passthrough 不回错误）；apps/gui/src/acp/acp-connection.ts:196-199（非 JSON 帧仅 console.warn 丢弃）
- 对标证据: t3code src/errors.ts:284-305（parseError/invalidRequest/methodNotFound/invalidParams 工厂）；src/protocol.ts:246-260（未知 ext 请求立即回 methodNotFound）
- 影响: 互操作排障困难，对端只能等超时而非立即得到协议级错误；解码失败无 wire 可观察性。
- 借鉴建议: **引入**。桥对可定位但非法的行回 -32700；GUI 对带 id 的未知方法回 -32601。

**F5【Minor】请求级取消/中断不传播，长回合 prompt 无保底结算**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:227-250（request 无中断通道）；:290-294（prompt `timeoutMs: null` 只由应答/断连结算）；:296-300（唯一取消手段是会话级 session/cancel 通知）
- 对标证据: t3code src/protocol.ts:399-403（Interrupt 消息路由）；src/client.ts:453-456（RPC 请求 id 从 1n<<32n 起与扩展请求 id 空间分区防碰撞）
- 影响: 挂死的单个请求无法请求级取消，只能断连或整会话 cancel；WS 活着但 agent 卡死时 GUI 永久 pending。
- 借鉴建议: **不引入完整 Interrupt 机制**（本仓为有序透传流，收益低于改造成本）；给 prompt 加保底超时 + 界面计时即可。

### 维度 2：能力面差距

**F6【Important】elicitation 能力整体缺失（协议交互 + UI + 能力协商三无）**
- 本仓证据: apps/ 全目录 grep `elicitation` 零命中；apps/gui/src/acp/acp-connection.ts:200-207（onRequest 分发无 elicitation 分支）；protocol.ts:170-176（clientCapabilities 无相关声明）
- 对标证据: t3code src/rpc.ts:97-101（ElicitationRpc）；src/client.ts:143-151（handleElicitation）；src/protocol.ts:284-303（elicitation_complete 通知解码）；meta.gen.ts:24-25
- 影响: 上游 agent 一旦发起 session/elicitation，GUI 无人应答，请求悬死到 agent 侧超时；能力协商位也不存在，未来接入必须动协议层。
- 借鉴建议: **引入**。最小闭环：onRequest 增加 elicitation 分支（表单渲染或显式 reject 应答）+ 能力位声明。

**F7【Important】session/load、session/fork、session/set_mode 缺失，恢复面只有 list/resume**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:302-308（仅 sessionList/sessionResume）；apps/ 全目录 grep `session/(load|fork)|set_mode` 零命中
- 对标证据: t3code src/rpc.ts:31-53（LoadSession/ForkSession/ResumeSession RPC 全套）；meta.gen.ts:12、17（session/load、session/set_mode）；src/client.ts:74-97
- 影响: 跨重启只能 resume，无法按名加载历史会话或分支会话；与设计 §5 拍板一致，但低于官方能力面，CLI/脚本消费场景受限。
- 借鉴建议: **引入 session/load**（收益直接、与既有持久化衔接）；fork/set_mode 视 dsh 子进程实际支持再定。

**F8【Minor】terminal 能力零协商零实现**
- 本仓证据: apps/gui/src/acp/protocol.ts:170-176（initializeParams 只声明 fs=false，无 terminal 位）；apps/ 全目录 grep `terminal/(create|output|kill|release)` 零命中
- 对标证据: t3code src/terminal.ts:6-25（AcpTerminal 句柄：output/waitForExit/kill/release）；src/rpc.ts:103-131（五个 terminal RPC）；src/agent.ts:392-429（createTerminal 返回类型化句柄）
- 影响: 「手机远程看 agent 跑命令」场景无法落地；但设计 §1.3 明示非目标，属知情缺口而非疏漏。
- 借鉴建议: **暂不引入**。登记为能力位缺口，待 GUI 有终端视图需求再开，届时直接参照 terminal.ts 句柄形状。

**F9【Minor】能力展示与输入面错位：agent 声明 image/audio 可用，GUI 永远只能发 text**
- 本仓证据: apps/gui/src/acp/protocol.ts:47-50（AgentCapabilities.promptCapabilities 含 image/audio，用于展示）；:42-45（AcpContentBlock 仅 `type:"text"`）；:201-207（promptParams 只组 text block）
- 对标证据: t3code src/_generated/schema.gen.ts（10375 行生成 schema 全量覆盖 content block 类型）
- 影响: GUI 按设计 §8「显示真实能力」展示 image=true，但输入面永不可发图，用户预期落空；半撒谎。
- 借鉴建议: **引入**（image content block 输入），或在能力展示处明确「本控制台暂不支持发送」。

### 维度 3：会话与状态管理

**F10【Important】抖动即拒权：90s 续连窗口只保 session/update，不保 outstanding 权限请求**
- 本仓证据: docs/design/acp-over-p2p-design.md:133-134（断流即对 outstanding request_permission 一律代答 reject-once）；apps/acp-agent/src/reattach.rs:43-71（UpdateCache 只缓存 update 行）；apps/gui/src/acp/acp-connection.ts:124-158（重连路径无权限请求状态恢复）
- 对标证据: t3code src/protocol.ts:169-176（断开时 failAllExtPending 显式失败所有 pending——同为失败语义，但其单进程模型无「重连后可补放」的问题域可借力）
- 影响: 移动网络抖一下，操作者正要点的批准按钮直接变「已拒绝」，agent 停摆需重新发起；续连体验的核心收益被权限路径抵消。
- 借鉴建议: **引入**（窗口内重连时补放未结算 permission 请求；grant 仍一次性、不落盘，不违反「永不持久化」红线）。

**F11【Minor】GUI 自动重连 3 次上限（约 7s）与桥 90s 续连窗口严重不匹配**
- 本仓证据: apps/gui/src/acp/acp-connection.ts:41-45（maxAttempts=3，1s/2s/4s 指数退避、15s 上限）；:145-149（用尽即 offline）
- 对标证据: t3code 无此层（单进程无重连问题）；本仓桥侧窗口为 90s（设计 §12-Q1、apps/acp-agent/src/reattach.rs 模块注释）
- 影响: 桥侧窗口还开着，GUI 已放弃重连，续连票据大概率作废、退化为 resume，窗口资源浪费。
- 借鉴建议: **引入**。重试节奏对齐窗口：窗口期内持续指数退避（cap 15s）直至 90s 用尽。

### 维度 4：测试策略

**F12【Important】契约测试无 spec 基准：mock 自造协议，故障注入维度少**
- 本仓证据: apps/gui/src/acp/mock-acp-ws.ts:148-163（mock 内手写方法分发，与真实 agent 行为无同源保证）；apps/acp-agent/src/bin/acp-echo-stub.rs（265 行 echo stub，无故障注入开关）；对比测试面本身不弱：36 个 gui 测试文件、apps/acp-agent/tests 9 个集成套件（loopback/reattach_window/permission_flow/security）、crates/acp-pump/tests 8 个
- 对标证据: t3code test/fixtures/acp-mock-peer.ts:9-16（env 开关注入 malformed 输出/立即退出码的**真实子进程**故障注入）、:57-80（mock agent 走 request_permission + elicit 全流程）；src/protocol.test.ts（652 行 58 断言，覆盖 wire 解码/路由/终止）
- 影响: 「真 ACP 语义」只有真机对拍笔记（apps/gui/src/acp/ndjson.ts:2-5 注释），无自动化回归；mock 与真 agent 的行为漂移已经产出 F2 的 session/delete 悬案。
- 借鉴建议: **引入**。把「真实子进程 mock + env 故障注入开关」模式搬进 acp_wave_e2e（malformed 行、mid-turn 退出码、elicitation 流程）；用官方 spec 样例帧做契约 fixture。

### 维度 5：可维护性

**F13【Important】无 schema/方法名生成管线，「什么是 ACP 方法」散落 5+ 处**
- 本仓证据: apps/gui/src/acp/protocol.ts（206 行全手写）；apps/acp-agent/src/mcp.rs:35（字符串匹配 `"session/new"`）、permission.rs:24（`method.ends_with("request_permission")` 后缀匹配）、reattach.rs:79-89（`"session/update"` 等谓词）；apps/gui/src/acp/acp-connection.ts（11 处方法名字面量）
- 对标证据: t3code package.json:38、44（`generate` 脚本 + @effect/openapi-generator）；scripts/generate.ts；src/_generated/schema.gen.ts（10375 行）+ meta.gen.ts（35 行方法/版本常量，标注 v0.11.3）
- 影响: spec 升级靠人肉多点同步；后缀匹配还会把扩展方法 `x/request_permission` 误分类进权限瀑布；F2 漂移即此机制缺失的直接后果。
- 借鉴建议: **引入**。哪怕只生成 TS 类型 + 方法名常量（对标侧 meta.gen.ts 仅 35 行，成本极低）；Rust 侧把方法判别收拢为 acp-common 单一模块。

**F14【Minor】协议类型无 spec 链接与版本标注，protocolVersion 硬编码**
- 本仓证据: apps/gui/src/acp/protocol.ts（全文无 agentclientprotocol.com 链接、无版本注）；:173（`protocolVersion: 1` 字面量）；:53（`protocolVersion?: number | string` 两可类型）
- 对标证据: t3code src/client.ts:42-132（每个 RPC 的 jsdoc 带 `@see https://agentclientprotocol.com/...` 精确锚点）；meta.gen.ts:2（`Current ACP schema release: v0.11.3`）、:35（`PROTOCOL_VERSION = 1`）
- 影响: 审阅者无法快速核对语义来源，spec 升级时无版本戳可比对。
- 借鉴建议: **引入**。文件头注版本 + 类型注挂 spec 锚点，半小时工作量。

---

## 分维度结论

| 维度 | 结论 |
|---|---|
| 1 协议层 | 本仓是「透传 + 少量手改」架构，协议正确性机制（校验/错误建模/方法单源）全面弱于对标；超时包络（30s/120s/慢速包络）反而比对标细 |
| 2 能力面 | 本仓 = list/resume/set_config_option/cancel；缺 elicitation、load/fork/set_mode、terminal、fs、image 输入 |
| 3 会话与状态 | 续连窗口是本仓独有优势，但窗口只护 update 不护权限请求，且 GUI 重连上限与窗口不匹配 |
| 4 测试 | 广度不输（36 gui 测试 + 17 rust 集成套件），缺「真实子进程 + 故障注入」契约基准与 spec fixture |
| 5 可维护性 | 对标有生成管线 + 版本戳 + spec 链接；本仓知识散落 5+ 处，漂移已现实发生 |

## Top5 行动建议（按 严重度 × 收益 排序，均可独立执行）

1. **修方法面漂移 + 方法名单源**（F2/F13 前半）：`session/delete`→`session/close`；在 apps/gui/src/acp 新建 methods 常量模块替换 11 处字面量。落点：apps/gui/src/acp（acp-connection.ts + protocol.ts）。半天。
2. **GUI 语义面 schema 解码层**（F1/F3）：zod 定义 SessionUpdate/PermissionRequest/错误码闭集，dispatch 失败计数 + 结构化日志。落点：apps/gui/src/acp/protocol.ts + acp-connection.ts。1-2 天。
3. **elicitation 最小闭环**（F6）：onRequest 增加 session/elicitation 分支（先支持显式 reject 应答 + 文本表单），initialize 申报能力位。落点：apps/gui/src/acp。2-3 天。
4. **续连窗口补放 outstanding 权限请求**（F10）：reattach 缓存扩为 updates + pending_permissions 两队列，窗口内重连补放；grant 仍一次性不落盘。落点：apps/acp-agent/src（reattach.rs + router/window.rs）。2-3 天，需安全拍板一次。
5. **真子进程契约测试**（F12）：给 acp-echo-stub 加 env 故障注入开关（malformed 行/exit code），e2e 新增 elicitation、malformed、mid-turn 崩溃三组用例；同步把 mock-acp-ws 的方法面改引 methods 常量。落点：apps/acp-agent/tests + apps/gui/src/acp。2 天。
