# /llm-share/offer/1 规范

状态：stable；自 2026-09-03；归属 crates/llm-share-offer；符合性：Core=声明信封签名与离线验签、TTL 失效、选路视图；Extended=rendezvous 在场宣告、声明发布命令面。

> 漂移标注：本页描述的协议流（get→应答信封）在产品常驻装配中尚未注册服务端 handler，
> 当前实现仅存在于冒烟工具与测试桩；发布通道的在场宣告实为 rendezvous 签名注册，
> 无服务端推送订阅。逐条见 §8；registry 的 impl=implemented 指 crate 能力面已落地。

## 1. 概览

出借方能力声明（offer）的发布与取回协议。声明本体是出借方签名的自述：模型清单、各模型声明闲量、账期截止、单请求上限、速率限额、声明 TTL、数据留存自述（retention）。借方取回信封后离线验签入簿，据此选路，再经 /llm-share/proxy/1 调用；/llm-share/redeem/1 兑换成功后也经本协议取回快照预填。

线上形态由两条腿组成（以代码为准）：

1. 在场宣告：发布方按声明 TTL 经 rendezvous 签名注册通道宣告在场与地址（专用 namespace `llm-share/offer/1`，签名覆盖，与会话发现租户隔离）；服务端注册 TTL 封顶 3600 秒，发布方周期刷新。
2. 信封交换：借方与本协议 ID 开流，写一帧 `get`，出借方回一帧 SignedOffer 信封 JSON。请求-响应各恰一帧，短连接，无会话态。

声明是软约束：spare/rate_limit 等字段仅供选路，实际授予以 proxy 预授权校验为准。

## 2. 线格式

### 2.1 帧序列

开流首帧为协议 ID（`/llm-share/offer/1` UTF-8，由开流写出）。其后：C→S 一帧 chunked 文本 `get`（4 字节，无 JSON 包络）；S→C 一帧 chunked JSON = SignedOffer 信封。载荷上限 1 MiB（1048576 字节）。应答不是合法 SignedOffer JSON 即取回失败，无独立错误帧。

### 2.2 声明本体（Offer）

| 字段（wire 名） | 类型 | 语义 |
|---|---|---|
| peer | string | 出借方 PeerId base58；必须与签名公钥推导 PeerId 一致 |
| models | string[] | 模型清单，非空且不重复 |
| spare | map&lt;model,u64&gt; | 声明闲量（token）；models 内每模型必须有正闲量条目 |
| period_ends | string | 账期截止日 YYYY-MM-DD |
| max_per_req | map&lt;model,u64&gt; | 单请求 max_tokens 上限；缺条目=未显式设限；不得引用未声明模型 |
| rate_limit | {rpm: u32, concurrency: u32} | 速率限额两项，均须为正 |
| ttl | u64 | 声明有效期（秒，wire 名为 ttl），须为正；过期即失效 |
| retention | string | 数据留存自述（如 "none"），透传不解释，供借方选路裁量 |

校验规则必须全执行：上述非空/正值/键归属约束任一不过即拒绝该声明；BTreeMap 键序保证 canonical 字节稳定。

### 2.3 签名信封（SignedOffer）

```json
{ "offer": <Offer>, "issued_at": 1725400000, "pubkey": "<b58 32B>", "sig": "<b58 64B>" }
```

签名前像 = canonical（offer）紧凑 JSON 后拼接 issued_at 小端 8 字节。验签必须依次执行：peer 与公钥绑定（公钥推导 PeerId 等于 offer.peer）、Ed25519 验签、时间窗 now ∈ [issued_at, issued_at+ttl)。任一不过即拒收；issued_at 在未来必须拒绝（NotYetValid）。

## 3. 时序与状态机

发布方：组装声明 → 以节点身份签名 → 原子落盘 `<data-dir>/llm-share/offer.json` →（装配了发布周期时）按声明 TTL 周期向 rendezvous 发签名注册刷新在场。声明更新即重新签发，issued_at 前移。

借方：rendezvous 查号得出借方地址集 → 对目标开流取回信封（单步建议超时 30 秒）→ 验签入簿（同 peer 重复发布覆盖旧条目，最新声明 wins）→ 选路。取回失败必须显式报错，不得静默使用旧声明。

TTL 失效语义：簿内声明按 expires_at = issued_at + ttl 过滤；查询视图只含未过期声明；过期清理（evict）必须逐条留 WARN 观测信号。失效由借方本地时间驱动，不存在服务端失效通知（与 /a2a/1 card 相的推送订阅不同，本协议没有订阅语义）。

选路视图（纯函数）：按目标模型过滤 → 剔除 TTL 已过期或该模型闲量为零者 → 按 spare 降序、同 spare 按 peer 字典序；不截断数量，重试与熔断由调用方决策。

## 4. 错误语义

本协议无服务端错误帧；错误全部表现为取回路径失败，借方按因显式报错：

| 因 | 判定 | 借方行为 |
|---|---|---|
| 开流失败 | 对端不支持本协议或不可达 | 报 OFFER-FAIL，换候选出借方 |
| 应答超时 | 单步超时（建议 30s）到 | 同上 |
| 应答解析失败 | 非 SignedOffer JSON | 同上 |
| PeerMismatch | peer 与公钥不绑定 | 拒收，视为冒名，不入簿 |
| BadSignature | 验签不过或载荷被改 | 拒收不入簿，留痕 |
| NotYetValid | issued_at 在未来 | 拒收（防时间戳攻击） |
| Expired | now >= issued_at+ttl | 不入簿或从视图剔除；清理时留 WARN |
| Encoding | canonical 编码失败 | 拒收 |

声明本体校验失败（§2.2 约束）发生在签发侧（必须拒签）与消费侧（必须拒收）两端。

## 5. 安全考量

信封即防伪与防篡改边界：peer 绑定防冒名、签名防篡改、TTL 窗口限时效、NotYetValid 防预签未来声明。声明为公开数据，无机密性要求。spare 与 rate_limit 是自述软约束，借方不得将其当作可用性承诺；真实准入由 proxy 三闸裁定。retention 是出借方单方自述，协议不验证其真实性。rendezvous 注册签名复用底座 sign_register 设施，防注册记录冒名。

## 6. 兼容与版本

Offer 顶层字段只增不改义，消费方必须忽略未知字段之外的加法（新字段进签名前像，旧验签方验签必败，属显式不兼容需升版）。rate_limit/ttl/retention 为已登记加法字段的先例。协议 ID 升版（/llm-share/offer/2）时新页新建，本页顶部标注取代关系。探测方式与 proxy 相同：开流写协议 ID，对端不支持即失败。

## 7. 测试向量

无（候选：offer-envelope.json 信封金样本与验签四态）。

## 8. 实现状态与出处

- 模型/信封/验签：crates/llm-share-offer/src/offer.rs（Offer/SignedOffer/VerifyError/PROTOCOL_ID）。
- 在场宣告与订阅簿：crates/llm-share-offer/src/publish.rs（OFFER_NAMESPACE/announce_register/OfferBook）。
- 选路：crates/llm-share-offer/src/route.rs（select_offers）。
- 发布/查看命令面：crates/p2p-cli/src/llm_share/offer.rs（publish/show，offer.json 原子写）。
- 借方取回客户端：crates/p2p-cli/src/llm_share/borrow_dial.rs（fetch_offer，get 帧约定）。
- 冒烟与测试桩服务端：scripts/ops/llm-share-smoke.sh（内嵌 OfferHandler：跳过协议 ID 首帧后回信封）、crates/p2p-cli/tests/llm_share_borrow_common/mod.rs。

漂移登记（代码为准，逐条）：

1. 协议流服务端 handler 未进产品装配：GUI 常驻 serve（apps/gui/src-tauri/src/llm_share/serve/mod.rs）仅注册 /llm-share/proxy/1 与 /llm-share/redeem/1；/llm-share/offer/1 的服务端实现当前仅存在于冒烟脚本内嵌进程与测试桩。产品节点间 borrow/redeem 成功后的 offer 取回依赖对端装配该 handler，现装配缺位。
2. 旧文档「已实现（能力声明发布通道）」口径：crate 层能力（常量、模型、签名信封、发布组装、订阅簿、选路、取回客户端）已落地且 stable；「发布通道」的线上形态是 rendezvous 签名注册 + 独立取回流，而非持续推送通道。
3. 设计文档 idle-token-sharing-plan §5 称 offer「亦可 mDNS 局域网广播」：代码未实现 offer 内容的 mDNS 广播（mDNS 仅作用于底座节点发现层），漂移。
4. 「TTL 失效订阅簿」无通知语义：OfferBook 为被动簿，失效 = 本地时间过滤 + evict 时 WARN 日志；不存在服务端失效推送或订阅登记。
5. 设计文档 §5.2 字段「溢价系数（预留）」未进入 wire（无该字段）；retention 为计划外新增字段（协调者拍板必含）。
6. OfferBook 无容量上限、无 issued_at 偏差钳制（未来时间戳由 NotYetValid 拒绝）；乱序到达的旧帧若仍在有效期内会覆盖新帧（源码注释明示 MVP 白名单场景可接受）。与 /a2a/1 的 AgentBook（容量 256 + 偏差 300s 钳制）不对齐。
7. rate_limit.concurrency 被 proxy 装配消费（max_concurrent）；rpm 声明字段当前无消费点。
8. rendezvous 注册 TTL 服务端封顶 3600 秒（底座侧约束），发布方声明 TTL 大于该值时在场宣告先于声明失效。
