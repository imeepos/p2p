# LLM 分享链接（Share Link）：聊天一键分享 LLM 额度 —— 设计方案 v2

> 状态: v2（四角度评审收敛：安全/协议/UX契约/架构，2026-09-08） | 日期: 2026-09-08
> v1 草案 → v2 的变更: 契约号 v12→v13（取代）；allowlist 数据模型 v2（共享锁/来源/到期）；
> serve 装配改 handle_inbound（弃 gate）；出借方幂等索引持久化；Claude 翻译状态机定稿；
> W 拆分修正（W0 契约冻结先行、p2p-cli allowlist 域归属）。
> 依赖: idle-token-sharing-plan.md（E10 三件套 Phase 0）、acp-share-design.md（分享链接模式先例）、
> gui-contract.md（§16 llm-share 命令面）、im-chat-design.md（聊天页宿主）
> 定位: E10 之上的加法应用层特性。不改通信内核，不改 /llm-share/proxy/1 既有帧结构。

## 1. 需求映射（用户原话 → 方案条目）

| 用户原话 | 现状 | 差距 | 本方案条目 |
| --- | --- | --- | --- |
| 需要填写 url key 协议类型，openai/claude 两协议都支持 | ProviderConfig（GUI localStorage）= name+baseUrl+apiKey+models，无协议字段；proxy 上游硬编码 OpenAI /chat/completions + Bearer | 协议类型字段与 UI；proxy 支持 Claude 协议（请求/SSE 翻译） | §5.1/§5.2（A 段） |
| 添加成功后可以通过聊天生成临时实用链接发给好友 | 分享=手动填 peerId+spare → offerPublish+allow（契约 v12），无链接 | 链接格式/台账/聊天生成与渲染/好友一键导入 | §5.3/§6（C 段） |
| 好用通过链接，通信用 P2P，用 LLM 提供方的机器发起请求并转发给使用者 | borrow 借方一次性拨号 P2P 调用已通；出借方 proxy 服务端只在 itest 装配，GUI 常驻 node 未接线；key/base_url 装配来源为空 | 出借方常驻 serve 接线（node_start 装配 + handle_inbound 取入站 peer）；provider 配置持久化到出借方本机 | §5.2/§5.4（B 段） |

## 2. 目标与非目标

### 2.1 目标（本波交付）

- G1 双协议：provider 表单可选 OpenAI / Claude；出借方按配置协议调上游，借方侧零改造
  （始终 OpenAI 兼容接口，协议差异由出借方代理翻译）。
- G2 聊天分享链接：出借方在分享面板生成 dsh-llm-share:// 临时链接，作为聊天消息发给好友；
  好友点击卡片一键导入即用（免手动 peerId/免手填 spare），模型范围由链接限定并由闸强制。
- G3 出借方常驻代理：GUI 节点启动即 serve /llm-share/proxy/1，按声明模型路由到配置上游
  （URL/Key/协议），P2P 转发响应；Key 永不出本机、不落 wire；重启保幂等（防 req_id 双记账）。
- G4 记账复用：借/贷流水、收据、幂等、冻结、争议窗口沿用 E10 账本，零新增记账语义。

### 2.2 非目标（显式排除）

- N1 不做全局市场/公开目录：链接只发给认识的好友，兑换=好友 PeerId 进 allowlist。
- N2 不做多上游聚合路由：Phase 0 仍买方选定卖方；一个 provider 配置对一组成声明模型。
- N3 不做非流式通道：SSE 流式是既有 proxy 通道，Claude 侧也走流式翻译。
- N4 不做群分享/批量分享、聊天页内生成入口一期不做（生成入口留分享面板）。
- N5 不改 E10 记账语义、不碰 p2p 内核 crate、不建冗余 ModelRoute.protocol 字段。
- N6 出借方账本全量持久化后置：本波只保幂等索引（§5.4 A3 决策），账本重放另议。

## 3. 现状盘点（评审者必读的事实基础）

- E10 三件套已合 main：llm-share-ledger / llm-share-offer / llm-share-proxy
  （/llm-share/proxy/1 三闸+SSE 逐帧转发+结算；ProxyConfig.allowlist: HashSet 装配期快照，
  server.rs:25-36；LenderProxy 账本/幂等索引全内存，server.rs:62）。
- proxy 上游抽象 Upstream trait（upstream.rs）：chat(&UpstreamCall) -> Result<SseByteStream,
  UpstreamFailure>；唯一实现 HttpUpstream 硬编码 POST {base}/chat/completions + Bearer。
- serve.rs 收据 upstream_hint 硬编码 "openai"（约 207 行）；extract_usage 只认同一 SSE 事件内
  顶层 usage.prompt_tokens + usage.completion_tokens（sse.rs）。
- GUI llm-share 九命令（gui-contract §16.1）；§16.3 v12 加法（2026-09-07 已合）：provider 配置
  仅存 GUI localStorage（键 p2p-gui-llm-providers）+ 分享动作=offerPublish+allow。本方案 v13 取代之。
- allowlist.json（p2p-cli allowlist.rs）：AllowEntry{models, note, granted_at}，peer_id 键单行，
  无来源/到期字段——撤销级联与授权到期无法落地。
- 出借方代理服务端仅 itest 装配（llm_share_common：ServeHandler::handle(stream) + InboundPeers
  gate 捕获入站 peer——旧接缝补丁，并发多借方会串线）；p2p-protocol 已有
  handle_inbound(peer, stream)（swarm serve 分发下传握手互认 PeerId），本方案采用之。
- ACP 分享链接先例（acp-share-design.md）：dsh-acp-share://v1?peer&addr&token&exp&sid、台账只存
  token 哈希、一次性=激活次数、聊天卡片（share-message-card.tsx 已定义未接线）+ useShareJoin。
- 聊天正文渲染唯一挂点：message-bubble.tsx TextWithShareLink（ACP 正则固定 dsh-acp-share 前缀，
  llm-share 需独立 scheme 分发）；BorrowPanel 无初始值入口（预填需 query 契约）。

## 4. 核心决策（评审收敛，冻结）

1. 协议翻译方向：借方始终 OpenAI 兼容（零改造、E10 契约不变）；出借方按 provider.protocol 选
   上游实现——openai → HttpUpstream；claude → ClaudeUpstream（请求/SSE 双向翻译）。
   ModelRoute 不加 protocol 字段：Upstream trait 增默认方法 protocol_hint() -> "openai"，
   ClaudeUpstream 覆写为 "claude"，serve 取 hint 填收据 upstream_hint（行为与实现同源）。
2. provider 配置归属：从 GUI localStorage 上移为出借方本机持久化
   （GUI 数据目录 llm-share/providers.json + 独立 0600 密钥文件 keys/<id>.key）；
   localStorage 一次性幂等迁移后删除旧键；契约 v13 显式取代 v12。
3. allowlist 数据模型 v2：AllowEntry += source: Option<String>（"share:<shareId>"，serde default）、
   expires_at: Option<u64>；ProxyConfig.allowlist 由 HashSet 快照改为 Arc<AllowlistGate>
   （RwLock<HashMap<peer, AllowEntry>>，admit 校验 peer + 模型集 + 到期）；
   allow/deny/兑换统一经共享句柄写（文件 + 内存同步）。兑换激活进单一互斥临界区
   （校验→写 allowlist→激活+1；写序 allowlist 先、激活后，崩溃窗口重试安全）。
4. 入站 peer 身份：ServeHandler 与 RedeemHandler 实现 ProtocolHandler::handle_inbound(peer,
   stream)，用握手互认的 PeerId（帧内自报一律不信）；GUI 不调 set_gate（gate 对出站拨号也生效，
   且并发多借方 last-writer-wins 串线）。
5. 出借方幂等持久化（本波最小）：serve 装配注入 persist sink，finalize 时把 Receipt append 到
   <data-dir>/llm-share/ledger.json；启动时从该文件重建 settled 索引（防 req_id 重放双记账）；
   账本全量重放后置（N6），serve 状态暴露「装配时快照 + 重启账本口径」提示。
6. 分享链接语义：链接=兑换凭证（token 128-bit CSPRNG hex，台账只存 sha256，原文只在创建响应
   出现一次）；兑换成功=该 PeerId 按分享模型集进 allowlist（source=share:<shareId>）；
   一次性激活固定=1（v1 不做 N 个不同 peer 各激活一次）；授权到期=链接 exp（默认 24h、上限 7d，
   allowlist 条目 expires_at 内缩）；撤销=置 revoked + 按 source 级联删 allowlist 条目；
   手工条目优先：redeem 不覆盖手工条目，同 peer 模型取交集。
7. 契约加法：gui-contract §16 升 v13（取代 v12：provider 存储/分享语义迁移；provider-share-form
   手动分享入口移除）；新命令面 8 条（§5.5）；CLI 对等 7 mapped + 1 exempt（serve_status，
   同 acp_console_status 先例）。
8. 任务编排：W0 契约冻结先行（本协调会话）→ W1（proxy Claude）∥ W2（llm-share-link crate +
   p2p-cli allowlist v2）→ W3（src-tauri 接线）∥ W4（GUI 视图，按冻结契约 + mock 并行）→
   W5b（E2E/冒烟/门禁）。合并序 W1→W2→W3→W4→W5b，协调者机械验收后 ff。

## 5. 方案细节

### 5.1 A 段：provider 协议类型（表单 + 存档）

- ProviderConfig 增加 protocol: "openai" | "claude"（旧存档缺省 openai，校验兼容）。
- 表单新增协议类型选择（radio，openai 缺省）；http:// baseUrl 显式告警（https 优先）。
- localStorage 迁移：读取旧键一次性迁移到 ProviderStore（幂等），成功后 removeItem 旧键；
  失败回滚不残留明文；迁移后写路径不再落明文键（Web 侧 provider-configs.ts 只做迁移源）。

### 5.2 A 段：proxy Claude 协议适配（crates/llm-share-proxy，W1）

- 新增 claude_upstream.rs：ClaudeUpstream 实现 Upstream trait（独立状态机）。
  - 请求翻译（OpenAI chat completions 体 → Claude /v1/messages 体）：
    system 抽取合并到顶层 system（多条按序合并）；messages 过滤 system、只留 user/assistant；
    model 直映；max_tokens 必填（wire ProxyRequest 已强制，保持）；stream: true 恒开；
    temperature/top_p 同名直映（二者并存时 top_p 优先并告警）；stop → stop_sequences；
    frequency_penalty/presence_penalty/logprobs/n/seed/response_format 无等价物→显式省略+告警；
    tool/tool_calls/tool 角色 → 一期拒绝（结构化错误，不静默字符串化）；
    thinking/非文本 content block → 忽略 + 告警（不静默丢失信号）。
  - 鉴权：x-api-key + anthropic-version: 2023-06-01。
  - baseUrl 归一：trim 尾斜杠；已以 /v1 结尾 → 拼 /messages；否则拼 /v1/messages。
  - 响应翻译（Claude SSE → OpenAI SSE，状态机跨 reqwest 字节块切分，不能假定一块一事件）：
    message_start 缓存 input_tokens（可发 assistant role 空 delta）；
    content_block_delta.text → data: {"choices":[{"delta":{"content":...}}]}；
    message_delta：合成 OpenAI usage chunk
    data: {"choices":[],"usage":{"prompt_tokens":<缓存input_tokens>,"completion_tokens":<output_tokens>}}
    （choices 必须为空数组）；stop_reason 映射 finish_reason（end_turn/stop_sequence→stop，
    max_tokens→length）；message_stop → data: [DONE]。
  - 错误：Claude 4xx/5xx → UpstreamFailure::Rejected(status)，日志脱敏记 type/message（禁 key）；
    2xx 非 SSE / 畸形 JSON / 缺 message_stop → UpstreamFailure::Broken（可观测）。
- Upstream trait 增默认方法 fn protocol_hint(&self) -> &'static str { "openai" }；
  HttpUpstream 缺省、ClaudeUpstream 返回 "claude"；serve.rs 取 hint 填 receipt.upstream_hint。
- 测试：进程内 mock Claude 上游，覆盖请求翻译 roundtrip、SSE 跨块切分、usage 合成、
  finish_reason 映射、baseUrl 归一、错误/断流映射、不支持的参数告警路径。

### 5.3 B 段：provider 配置持久化 + 出借方常驻 serve（W2 类型 + W3 接线）

- ProviderStore（GUI 侧 src-tauri/src/llm_share/provider_store.rs，W3）：
  - providers.json（version=1 信封，tmp+rename 原子写；只存 id/name/baseUrl/protocol/models/
    createdAt + apiKeyRef，不存明文 key；目录 0700）。
  - 密钥文件 keys/<providerId>.key，0600（keystore.save 修复：已存在文件显式 chmod 0600 +
    tmp+rename 原子写，B 加固项）；provider_remove 级联删 key 文件。
  - apiKey 入参仅经 IPC（GUI）或 stdin/env（CLI），禁 argv。
- 出借方 serve 装配（src-tauri/src/llm_share/serve.rs，W3）：
  - node_start 时（chat install 之后）若 offer.json 有效且 providers 匹配则装配
    LenderProxy + ServeHandler/RedeemHandler（均 handle_inbound 实现）；装配前查
    protocols() 断言 /llm-share/* 未注册（防静默覆盖）。
  - 模型路由：offer.models 中每模型 → 唯一 provider（模型→provider 唯一映射，冲突装配即拒+告警）。
  - allowlist 共享句柄：AllowlistGate（Arc<RwLock<HashMap<String, AllowEntry>>>）由 serve 持有，
    allow/deny/share_redeem 命令与 admit 走同一句柄（文件 + 内存同步）。
  - 幂等持久化：finalize 时 Receipt append 到 <data-dir>/llm-share/ledger.json（0600），
    启动重建 settled 索引。
  - node_stop 卸载；装配失败=可查询 serve 状态（RunningNode.serve_status: assembled + reason），
    不阻断节点启动（可观测非静默）；装配输入是启动快照（offer/providers 运行中变更不热更，
    状态暴露快照提示）。
- 契约 §16 v13 命令面 8 条（§5.5）。

### 5.4 C 段：分享链接全链（crates/llm-share-link，W2）

- 链接格式（冻结契约，对齐 ACP 先例，scheme 独立）：

      dsh-llm-share://v1?peer=<base58PeerId>&addr=<addr>&token=<32hex>&exp=<unix秒>&sid=<shareId>&models=<model1,model2>

  - addr 可重复（QUIC/TCP/中继多地址）；token=128-bit CSPRNG hex；exp/sid 展示性提示，
    权威判定在出借方；models 必填非空（缺省=不限模型会全模型放行，禁）且须 ⊆ 当前 offer.models。
  - 解析规则：scheme 不符拒绝；peer/token 缺失拒绝；token 非 32-hex 拒绝；未知参数忽略。

- 分享台账 <app-data>/llm-share/shares.json（version=1 信封，tmp+rename 原子写，目录 0700）：
  - ShareEntry{share_id, token_sha256, provider_id, models, max_activations(=1),
    activations, expires_at_unix, revoked, note, created_at, bound_peer}。
  - 兑换激活（纯逻辑，锁内 read-check-write）：存在/未撤销/未过期/未超次/（未绑定 或
    绑定者=本 peer）→ 产出兑换结果{peer, models, source:share:<id>, expires_at}，
    allowlist 写入由装配方注入回调执行（llm-share-link 不直接写 allowlist.json，
    规避 p2p-cli 文件域）；同 peer 已绑定二次兑换=幂等成功（不重复计数，对齐 ACP §4）。

- 兑换协议 /llm-share/redeem/1（协议 ID 登记 wire-protocol §3.2 + builtin-and-versioning）：
  - 帧：请求 {token}（JSON），响应 {ok: true} 或结构化错误码
    （share-revoked/expired/exhausted/bound-other/invalid）；零流水。
  - 借方经 Node::request(peer, redeem_protocol, json{token}, timeout)（request-response 原语）；
    lender RedeemHandler 实现 handle_inbound(peer, stream)，身份一律取认证 PeerId。
  - 应答不内嵌 offer 快照——向导预填复用 borrow_dial::fetch_offer（已测已合），
    避免应答 schema 与 SignedOffer 耦合。
  - 限流/幂等：按认证 PeerId 限流 + 全局失败计数冷却；失败响应包络统一不外泄 share 是否存在。

- 命令面（GUI+CLI 对等，W3/W4）：share_create / share_list / share_revoke / share_redeem（§5.5）。

### 5.5 契约 §16 v13 命令面（8 条新增，风格对齐现有 9 命令：camelCase、必填显性报错、业务拒绝非 Err）

| 命令 | 参数 | 返回 | 语义 |
| --- | --- | --- | --- |
| llm_share_provider_list | - | { providers: LlmProviderView[] } | apiKey 只出掩码；损坏存档=显式报错回空不静默 |
| llm_share_provider_save | config{id?, name, baseUrl, protocol:openai\|claude, apiKey, models[]} | LlmProviderView | id 缺省生成；apiKey 明文仅入参落 0600 密钥文件；name/baseUrl/models 必填显性报错 |
| llm_share_provider_remove | providerId | { removed: true } | 不存在=显式报错非错误态（对齐 deny 语义）；级联删 key 文件 |
| llm_share_share_create | req{providerId, models?, expiresAt?, maxActivations?, note?} | { link, shareId, expiresAt, models } | models 缺省=provider 全模型且须 ⊆ offer.models；maxActivations 固定 1；token 原文只在这条响应出现一次 |
| llm_share_share_list | - | { shares: LlmShareEntry[] } | 永不含 token/明文 key；status 推导（active/expired/revoked/exhausted） |
| llm_share_share_revoke | shareId | { revoked: true } | 按 source=share:<id> 级联删 allowlist 条目；不存在/已撤销=显式报错 |
| llm_share_share_redeem | link | LlmShareRedeemResult{offer:{peer,models,spare,periodEnds}, shareId, owner} | 业务拒绝码 share-revoked/expired/exhausted/bound-other/invalid 原样透出非 Err；scheme/peer/token 缺失=参数错误显式 Err |
| llm_share_serve_status | - | LlmServeStatus{ assembled, providerId?, models[], lastError? } | assembled:false 是常态非故障；lastError 供面板显式告警 |

同步面：ipc-types.ts IpcBackend + src-tauri commands.rs + lib.rs generate_handler + views/llm-share
types.ts LlmShareBackend + backend.ts live invoke + lib/mock-llm-share.ts（拆分后）+ cli-parity.tsv
（7 mapped：llm-share provider|share ...，逻辑进 crates/p2p-cli 共享事实源；1 exempt: serve_status
带 reason）+ docs/ops/p2pctl-ai-guide.md AI-DOCS-SYNC 条目。

### 5.6 D 段：E2E 与文档（W5b）

- itest 扩展（crates/p2p-itest）：双节点 + 进程内 mock OpenAI/Claude 上游 +
  链接生成→兑换→borrow 全链 + 双借方并发归属（handle_inbound）+ 重启幂等（重建 settled 索引后
  重放 req_id 被拒）；ServeHandler 升 handle_inbound。
- 契约 §16 v13 落 main（W0）；协议 ID 登记 wire-protocol §3.2 + docs/protocol/builtin-and-versioning.md
  §1.2；docs/ops/p2pctl-ai-guide.md 九条目同步；冒烟文档 docs/ops/llm-share-smoke.md 补充双协议/分享链路。

## 6. 交互流（使用者视角）

    出借方 A（配置了 provider，发布 offer）
      1. 分享面板选 provider → 生成 dsh-llm-share:// 链接（模型范围/有效期/一次性）
      2. 链接作为聊天消息发给好友 B（或复制粘贴）
    好友 B
      3. 聊天卡片点击「用链接加入」→ share_redeem(link)（拨号 /llm-share/redeem/1 兑换）
      4. A 侧 allowlist 落 share 条目（source=share:<id>，模型集限定，到期=exp）→ 返回成功
      5. 向导拉 offer 快照（borrow_dial::fetch_offer）预填 targetPeer=A/model=分享模型 → 输入问题
      6. P2P 流：B→A /llm-share/proxy/1 → A 按 provider.protocol 调上游（OpenAI 直发/Claude 翻译）
         → SSE 逐帧转发回 B → 收据+账本（E10 原样）+ 幂等索引持久化

## 7. 安全与合规（评审收敛，红线）

- apiKey：仅出借方本机 0600 落盘（keystore.save 修复 + tmp+rename）；不落 wire、不进日志
  （make check 新增 grep 门禁：禁 apiKey/token 变量直接进 log 宏插值，强制 redact 辅助函数）、
  不进 offer/台账/链接；http:// baseUrl 显式告警；provider_remove 级联删 key。
- share token：128-bit CSPRNG；台账只存 sha256；原文只在创建响应出现一次；
  链接=凭证以聊天消息落两侧存储——文档声明「聊天存储=凭证存储」信任边界，
  分享面板「分享即授权」确认；exp 默认 24h 上限 7d；激活=1、兑换后立废、绑定首 peer。
- 兑换：身份取认证 PeerId（handle_inbound），帧内自报不信；锁内 read-check-write 防 TOCTOU；
  按 peer 限流 + 失败冷却；失败包络统一；审计 share-redeemed/reuse-denied/expired/revoked/exhausted。
- allowlist：默认拒绝不变；手工优先、redeem 不覆盖手工条目、同 peer 模型取交集；
  revoke 按 source 级联；到期经 admit 惰性清理。
- 记账：share 授权不绕过冻结/净差上限（闸 3 不变）；净差兜底=出借方全局 net_limit（接受上界 +
  短 TTL + 审计）；重启保 req_id 幂等（双记账红线）。
- 合规：不翻墙/不做开放市场不变；文档声明「链接扩散=放弃邀请边界」既定接受度 +
  TTL/次数/模型/净额四约束收窄；兑换时校验 peer 是否在好友簿/聊天记录作软提示（不做硬闸）。

## 8. 验收口径（机械判定，评审后冻结）

- B1 双协议 roundtrip：openai 直发与 claude 翻译后 SSE 语义一致（itest mock 断言；
  usage 必须经 message_start input_tokens + message_delta output_tokens 合成命中 extract_usage）。
- B2 provider 管理：save/list/remove + stat keys/<id>.key == 0600 + 存档损坏显式报错 +
  provider_remove 级联删 key。
- B3 常驻 serve：node_start 后 /llm-share/proxy/1 双节点 itest 拨通；serve_status.assembled/
  lastError 可断言；未发布 offer 或 provider 缺失=assembled:false + lastError，节点照常启动。
- B4 链接全链：share_create → 聊天文本发送 → share_redeem → allowlist 出现 source=share:<id>
  条目（模型集/到期正确）→ borrow 成功；二次兑换被拒（bound-other/幂等成功二选一按语义）；
  revoke 后兑换被拒 + 级联条目移除；过期兑换被拒。
- B5 密钥与 token 泄露面（机械替代）：itest 断言 P2P 帧与上游 mock 收到的 HTTP 请求体不含
  apiKey 明文（仅允许出现在对上游鉴权头）；0600 stat；share_list 返回体不含 token、
  share_create 响应 token 仅一次且与链接一致；make check 新增 grep 门禁禁 key/token 进 log 插值。
- B6 双借方并发归属：两借方同时拨号，各自账本/收据归属正确（handle_inbound，无 gate 串线）。
- B7 重启幂等：serve 重启重建 settled 索引后，同 req_id 重放被 DuplicateReqId 拒（双记账红线）。
- B8 GUI 门禁：pnpm test/build/check:i18n + cli-parity + ai-docs-sync 全绿。
- B9 Rust 门禁：cargo test --workspace + clippy -D warnings + make check 全绿。

## 9. 任务拆分与并行（评审收敛定稿）

- W0 契约冻结（本协调会话，先行）：gui-contract §16 v13（取代 v12）+ p2pctl-ai-guide 条目 +
  协议 ID 登记（wire-protocol §3.2 + builtin-and-versioning §1.2）+ 方案文档 v2。
- W1 proxy Claude 适配（crates/llm-share-proxy/**，域互斥，可与 W2 并行）。
- W2 llm-share-link crate + p2p-cli allowlist v2（新增 crates/llm-share-link/** + 根 workspace
  members + crates/p2p-cli/src/llm_share/allowlist.rs source/expires_at，可与 W1 并行）。
- W3 src-tauri 接线（provider_store + serve 装配 + AllowlistGate + 幂等持久化 + 8 命令；
  依赖 W1+W2 契约面冻结后动码）。
- W4 GUI 视图（provider 表单协议字段 + 分享面板 + 聊天卡片 + 预填 query + i18n/menu.def/App.tsx
  登记 + localStorage 迁移；按 W0 冻结契约 + mock-ipc 并行推进）。
- W5b E2E + 冒烟 + 门禁收尾（依赖 W3+W4；含 handle_inbound 升级、双借方并发、重启幂等用例）。

每个 W 独立 worktree/分支，协调者机械验收后 ff 合并 main，收尾四步；中央登记文件
（menu.def.ts/App.tsx/i18n types+locale）改动压独立小提交；allowlist.rs 属 p2p-cli 域归 W2。

