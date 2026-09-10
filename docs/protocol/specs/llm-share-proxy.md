# /llm-share/proxy/1 规范

状态：stable；自 2026-09-04；归属 crates/llm-share-proxy；符合性：Core=三闸准入、SSE 逐帧转发、req_id 幂等、预授权结算与签名收据、错误码全表；Extended=claude 上游翻译路由、authz 判定源装配。

## 1. 概览

闲置 LLM 额度共享的代理调用协议。借方（B）以 OpenAI chat completions 语义向出借方（A）发请求，A 校验准入后注入自有上游 key 转发，把上游 SSE 流逐帧转回 B，并按实际 usage 完成双边记账（barter），以 A 签名收据为记账唯一凭据。协议目标见 idle-token-sharing-plan §4/§5；上游 key 只存在于出借方进程，B 不接触上游。

与相邻协议关系：/llm-share/offer/1 提供出借前的能力声明；/llm-share/redeem/1 提供链接兑换激活（写准入表）；本协议是唯一产生记账流水的协议。

传输复用底座帧封装与 chunked 通道（wire-format.md），本页只写 payload 语义。流内身份取底座握手认证的 PeerId，帧内自报身份不得采信。

## 2. 线格式

### 2.1 请求帧

开流后首帧为协议 ID（`/llm-share/proxy/1` UTF-8，底座标准开手）。兼容两种接线：裸流装配（首帧协议 ID，随后 chunked 请求帧）与入站分发已消费协议 ID（首帧即 chunked 请求帧本体）。实现以首字节判别（`/` 为协议 ID；0x00-0x02 为 chunked 类型头），不得猜测降级。

请求帧载荷为 UTF-8 JSON 对象：

| 字段 | 类型 | 必填 | 语义 |
|---|---|---|---|
| req_id | string | 是 | 借方生成的幂等键（UUID 建议）；空即 BadRequest |
| model | string | 是 | 模型名；须命中出借方模型路由表 |
| max_tokens | u64 | 是 | 输出生成上限；兼容取 max_completion_tokens；0 即 BadRequest |
| body | object | 否 | OpenAI chat completions 请求体；缺省时整个对象即 body（扁平形态） |
| wire_bytes | usize | 否 | 请求帧线字节长度；服务端以实际收到字节数计算，此字段仅供参考 |

嵌套形态（含 body 字段）与扁平形态（OpenAI 字段 + 顶层 req_id/model/max_tokens）都必须接受。发往上游前必须剥离 body 内的 req_id（上游不识别）。

### 2.2 应答帧序列

应答为一个 chunked 消息序列：若干 Sse 帧 + 恰一帧终结（Done 或 Error）。载荷为 UTF-8 JSON，以 `t` 字段区分：

| t | 字段 | 语义 |
|---|---|---|
| sse | d: string | 上游 SSE 事件原文（一帧一个完整事件，含 `data:` 行） |
| done | receipt: Receipt | 结算完成，正常终结 |
| error | code, message, receipt? | 终结失败；断流计费时 receipt 携带估算收据 |

### 2.3 收据（Receipt，llm-share-ledger 定义）

| 字段 | 类型 | 语义 |
|---|---|---|
| v | u8 | 恒 1 |
| req_id | string | 幂等键，与请求一致 |
| period | string | 出借方本地账期（如 "2026-09"） |
| lender / borrower | string | 双方 PeerId base58 文本 |
| model | string | 模型名 |
| usage | {input: u64, output: u64} | 上游 usage；断流时为估算值 |
| estimated | bool | true=usage 为估算（断流未获上游 usage） |
| upstream_hint | string | 上游协议标识："openai" 或 "claude" |
| ts | u64 | 收据 Unix 秒时间戳（争议窗口起点） |
| sig | string | Ed25519 签名 base58 |

签名语义：sig 为除 sig 外全字段的规范化 JSON（serde_json 键排序、无空白）的 Ed25519 签名，base58 编码。验签方必须以出借方公钥（即 lender PeerId 推导公钥）验证；任何字段篡改必须导致验签失败。

## 3. 时序与状态机

正常事务：B 开流并写请求帧；A 读帧后依次执行准入判定（§3.1）；全部通过则调用上游并逐帧转发 Sse；流结束后结算（解冻、双边流水入账、签收据）并写 Done 帧；B 验签后入账。全流程一条流，终结帧写出后流关闭。

### 3.1 准入判定序（以代码为准，逐步短路）

1. 闸1 身份准入：判定源为 authz.check(peer, llm.borrow)（装配 authz 判定源时）；未装配时回落 allowlist 集合判定。拒绝码恒 not_allowlisted，判定来源与原因只进 message 与审计日志。存储读失败必须按拒绝处理。
2. 模型白名单：req.model 未命中路由表即 model_not_served。
3. req_id 幂等：已结算（settled）重放回 duplicate_req_id 并附原收据；在途（pending）重放同样拒绝。
4. 并发闸：出借方全局并发许可耗尽即 concurrency_exceeded（上限=max_concurrent，最小 1）。
5. 闸3 预授权冻结：est = ceil(wire_bytes/4) + max_tokens；借方净差欠额 + 该账期全部在途冻结 + est 超过净差上限即 freeze_insufficient。净差上限由装配方按声明 spare 折算（LimitPolicy 默认 spare 万分比 5000 即 50%，可叠加绝对封顶）。冻结失败必须清除该 req_id 的在途标记。

前三步失败、并发失败、冻结失败均不调用上游、不产生流水。

### 3.2 req_id 状态

状态集合 {无, pending, settled}。迁移：无→pending（准入第 3 步通过）；pending→settled（结算成功）；pending→无（冻结失败或上游连接失败，可重试）；settled 恒久保留（重放回传原收据）。

### 3.3 断流

两个断流入口：上游流中途错误；B 中途离场（Sse 帧写失败，视同断流，上游 token 已实际消耗仍须计费）。断流时已完整收到的 SSE 事件必须已转发，残余半截事件计入估算口径后丢弃。

## 4. 错误语义

错误码为 snake_case 字符串，承载于 Error 帧 code 字段。全表（收=借方，发=出借方）：

| code | 触发 | 发方行为 | 收方行为 |
|---|---|---|---|
| bad_request | 请求帧非 JSON / 缺 req_id、model、max_tokens | 回 Error 后关流，上游零调用 | 修正请求后重开流 |
| not_allowlisted | 闸1 拒绝（含 authz 全部 Deny reason 与存储读失败） | 回 Error；审计留判定来源 | 停止重试该出借方 |
| model_not_served | 模型不在路由表 | 回 Error | 换模型或换出借方 |
| duplicate_req_id | req_id 已结算或在途 | 回 Error；settled 重放必须附原收据 | 以附带回的收据为准，不重复记账 |
| concurrency_exceeded | 并发许可耗尽 | 回 Error | 退避重试 |
| freeze_insufficient | 预授权冻结失败 | 回 Error；清 pending | 缩减 max_tokens 或结算旧欠 |
| upstream_rejected | 上游连接期失败或 HTTP 非 2xx（状态码进 message） | 解冻、清 pending，回 Error，零流水 | 可换出借方重试 |
| upstream_stream_broken | 上游流中断且无 usage | 按估算结算（estimated=true），回 Error 且必须附估算收据 | 验签收据；在争议窗口内可申诉 |
| settle_failed | 实际 usage 超冻结额 | 冻结保留待人工处置，回 Error，不入账 | 联系出借方处置 |
| internal | 收据入账失败等内部错误 | 回 Error；错误必须留 ERROR 级日志 | 退避重试 |

Error 帧携带 receipt 仅限两种：upstream_stream_broken（估算收据）与 duplicate_req_id 的 settled 重放（原收据）；其余 Error 帧不得携带 receipt。B 收到终结帧前流中断视为传输失败，本地不得入账。

### 4.1 usage 提取与估算

usage 只认 SSE 事件内 `usage.prompt_tokens` / `usage.completion_tokens`（OpenAI 形态）；流末携带者覆盖先前值。断流无 usage 时按字节估算：input = ceil(请求帧字节数/4)，output = ceil(已转发字节数/4)。估算收据必须标 estimated=true。

### 4.2 争议窗口

窗口自收据 ts 起算：普通收据 86400 秒（24h），estimated 收据 259200 秒（72h）。状态机 Pending → Disputed（窗口内提出）| Finalized（届满无争议终局入账）。超窗、重复争议、未届满终局都必须拒绝。

## 5. 安全考量

上游 key 仅存出借方进程内存；落盘必须 0600（读取时发现权限过宽必须收紧）。B 的 prompt 以明文经 A 转发（代理模型下端到端加密不成立），传输机密性由底座 QUIC/Noise 保证；A 不得落盘 prompt，仅留元数据（双方 PeerId、模型、usage、耗时、终态）。准入身份取握手认证 PeerId。冻结硬闸防超长 context 轰炸烧穿闲量；并发闸与净差上限构成资源防线。收据验签失败必须显式报错，不得静默接受。

## 6. 兼容与版本

加法不破坏：请求新字段必须忽略（未知字段不拒）；ErrorCode 只增不改义；新增错误码接收方必须能按未知错误码显式失败。协议语义不兼容变更时升版为 /llm-share/proxy/2 并存过渡。探测方式：开流写协议 ID 首帧，对端不支持即流被拒，无独立探测帧。

## 7. 测试向量

无（候选：proxy-request.json 请求帧双形态与边界；proxy-frames.json 应答帧序列与估算收据）。

## 8. 实现状态与出处

- 帧与协议 ID：crates/llm-share-proxy/src/wire.rs（ProxyRequest/ProxyFrame/PROTOCOL_ID、双接线判别）。
- 准入判定序：crates/llm-share-proxy/src/server.rs（admit）；闸1 判定源切换 crates/llm-share-proxy/src/gate1.rs（authz.check(llm.borrow)，allowlist 转只读归档）。
- 流编排与结算：crates/llm-share-proxy/src/serve.rs（forward/finalize/abort）。
- SSE 切分与 usage 提取、估算口径：crates/llm-share-proxy/src/sse.rs。
- 上游抽象与 openai 直发：crates/llm-share-proxy/src/upstream.rs、upstream_http.rs（连接超时 10s）；claude 翻译 crates/llm-share-proxy/src/claude_upstream.rs 及 response.rs。
- 收据/冻结/账本/争议窗：crates/llm-share-ledger/src/{receipt,hold,ledger}.rs。
- 客户端（借方）：crates/llm-share-proxy/src/client.rs（收据强制验签）。
- 产品装配：apps/gui/src-tauri/src/llm_share/serve/{mod,proxy,replay}.rs。

漂移登记（代码为准）：

1. 幂等索引（settled/pending）为进程内存态；纯 crate 层不持久。产品装配层以出借侧收据文件重建 settled 索引防重启双记账（serve/replay.rs rebuild_settled），仅收本机 lender 收据、逐条验签。
2. 门禁前置回放件（serve/replay.rs）以 CHUNK+END 两帧重装已读请求帧再委托 LenderProxy::serve；协议语义不变，属装配层实现细节。
3. claude 翻译上游为 Extended 路由：tools/tool_choice/tool 角色在翻译期显式拒绝（归 upstream_rejected），frequency_penalty 等 6 字段显式省略并告警，不静默。
4. 争议窗口与 DisputeTracker 位于 llm-share-ledger，协议页 §4.2 为其线外语义；协议流本身不承载争议帧，争议为双方本地账本操作。
5. 净差上限默认值（spare 万分比 5000）是库默认，装配方可以覆盖；协议不固定该值。
