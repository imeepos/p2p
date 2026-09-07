# LLM 分享链接（Share Link）：聊天一键分享 LLM 额度 —— 设计方案 v1

> 状态: v1 草案（评审轮前） | 日期: 2026-09-08
> 依赖: idle-token-sharing-plan.md（E10 三件套 Phase 0）、acp-share-design.md（分享链接模式先例）、
> gui-contract.md（§16 llm-share 命令面）、im-chat-design.md（聊天页宿主）
> 定位: E10 之上的加法应用层特性。不改通信内核，不改 /llm-share/* 既有帧结构。

## 1. 需求映射（用户原话 → 方案条目）

| 用户原话 | 现状 | 差距 | 本方案条目 |
| --- | --- | --- | --- |
| 需要填写 url key 协议类型，openai/claude 两协议都支持 | ProviderConfig（GUI localStorage）= name+baseUrl+apiKey+models，无协议字段；proxy 上游硬编码 OpenAI /chat/completions + Bearer | 协议类型字段与 UI；proxy 支持 Claude 协议（请求/SSE 翻译） | §5.1/§5.2（A 段） |
| 添加成功后可以通过聊天生成临时实用链接发给好友 | 分享=手动填 peerId+spare → offerPublish+allow，无链接；ACP 已有 dsh-acp-share:// 链接+台账+聊天卡片先例 | 链接格式/台账/聊天生成与渲染/好友一键导入 | §5.3/§6（C 段） |
| 好用通过链接，通信用 P2P，用 LLM 提供方的机器发起请求并转发给使用者 | borrow 借方一次性拨号 P2P 调用已通；出借方 proxy 服务端（LenderProxy serve）只在 itest 装配，GUI 常驻 node 未接线；key/base_url 装配来源为空 | 出借方常驻 serve 接线（node_start 时装配 + gate 捕获入站 peer）；provider 配置持久化到出借方本机 | §5.2/§5.4（B 段） |

## 2. 目标与非目标

### 2.1 目标（本波交付）

- G1 双协议：provider 表单可选 OpenAI / Claude；出借方按配置协议调上游，借方侧零改造
  （始终 OpenAI 兼容接口，协议差异由出借方代理翻译）。
- G2 聊天分享链接：出借方在聊天页/分享面板生成 dsh-llm-share:// 临时链接，作为聊天消息
  发给好友；好友点击卡片一键导入即用（免手动 peerId/免手填 spare）。
- G3 出借方常驻代理：GUI 节点启动即 serve /llm-share/proxy/1，按声明模型路由到
  配置的上游（URL/Key/协议），P2P 转发响应；Key 永不出本机、不落 wire。
- G4 记账复用：借/贷流水、收据、幂等、冻结、争议窗口全部沿用 E10 账本，零新增记账语义。

### 2.2 非目标（显式排除）

- N1 不做全局市场/公开目录：链接只发给认识的好友，兑换=好友 PeerId 进 allowlist。
- N2 不做多上游聚合路由：Phase 0 仍买方选定卖方；一个 provider 配置对一组成声明模型。
- N3 不做非流式通道：SSE 流式是既有 proxy 通道，Claude 侧也走流式翻译。
- N4 不做群分享/批量分享：一期单对单聊天内分享。
- N5 不改 E10 记账语义、不碰 p2p 内核 crate。

## 3. 现状盘点（评审者必读的事实基础）

- E10 三件套已合 main：llm-share-ledger（收据/流水/幂等/冻结/争议）、llm-share-offer
  （能力声明/TTL/选路）、llm-share-proxy（/llm-share/proxy/1 三闸+SSE 逐帧转发+结算，
  见 crates/llm-share-proxy/src/server.rs ProxyConfig/ModelRoute）。
- proxy 上游抽象 Upstream trait（crates/llm-share-proxy/src/upstream.rs）：
  chat(&UpstreamCall) -> Result<SseByteStream, UpstreamFailure>；唯一实现
  HttpUpstream（upstream_http.rs）硬编码 POST {base}/chat/completions + Bearer。
- GUI llm-share 九命令（gui-contract §16.1）：offer_publish/show、allow_list/allow/deny、
  borrow、ledger_list/balance、receipt_verify；src-tauri/src/llm_share/* 薄封装 p2p_cli::llm_share。
- ProviderConfig（apps/gui/src/views/llm-share/provider-configs.ts）仅 localStorage；
  分享表单（provider-share-form.tsx）= 手动填 peerId+spare → offerPublish+allow。
- 出借方代理服务端仅在 itest 装配（crates/p2p-itest/tests/llm_share_common/mod.rs：
  ServeHandler+InboundPeers gate 捕获入站 peer）；GUI 常驻 node（src-tauri/src/state.rs
  node_start → build_node → handle_protocol(echo) + chat install）未接 llm-share serve。
- ACP 分享链接先例（docs/design/acp-share-design.md）：dsh-acp-share://v1?peer&addr&token&exp&sid、
  台账只存 token 哈希、一次性=激活次数、聊天卡片（share-message-card.tsx）+ useShareJoin 导入。
  本方案链接格式/台账语义/卡片交互对齐该先例，标识符独立（llm-share 域）。

## 4. 核心决策（讨论后定稿，评审重点）

1. 协议翻译方向：借方始终 OpenAI 兼容（零改造、E10 契约不变）；出借方按 provider.protocol
   选上游实现——openai → 现有 HttpUpstream；claude → 新增 ClaudeUpstream（请求 OpenAI→Claude、
   响应 Claude SSE→OpenAI SSE 翻译）。理由：客户端面冻结、G1 由出借方单点承担。
2. provider 配置归属：从 GUI localStorage 上移为出借方本机持久化
   （GUI 数据目录 llm-share/providers.json，apiKey 独立 0600 密钥文件，JSON 只存掩码/引用）。
   出借方 serve 装配时按 providerId 读配置建 ModelRoute。localStorage 旧数据一次性迁移。
3. 分享链接语义：链接=兑换凭证（token 128-bit 随机 hex，台账只存哈希），兑换成功=该
   PeerId 进 allowlist（限定分享的模型集）+ 提供 offer 声明所需信息（models/spare/period
   由出借方在生成时从当前 offer 快照带入，好友侧不必手填）。一次性=激活次数（默认 1），
   绑定首次激活的 PeerId。撤销=置 revoked + 级联移除 share 来源的 allowlist 条目。
4. 出借方 serve 生命周期：跟随 GUI node_start；装配 LenderProxy 需 allowlist/models 源
   ——以 offer.json（已发布声明）+ providers.json（上游配置）为装配输入，节点停止即卸载。
   入站 peer 身份经 ConnectionGate 捕获（沿用 itest InboundPeers 模式，GUI 当前未用 gate，
   属 add-only 接线，不改变既有连接语义）。
5. 链接生成入口：分享面板（provider 级「生成分享链接」）+ 聊天页（命令/入口产链接，
   复制或直接作为文本消息发送）。聊天正文识别 dsh-llm-share:// 渲染卡片，点击走导入。
6. 契约加法走 §16 扩容：新增命令面（provider 管理 + share 管理 + serve 状态），
   冻结后 GUI/CLI 按同一契约实现，cli-parity/ai-docs 守卫同步。

## 5. 方案细节

### 5.1 A 段：provider 协议类型（表单 + 存档）

- ProviderConfig 增加 protocol: "openai" | "claude"（旧存档缺省 openai，校验函数兼容）。
- 表单新增协议类型选择（radio，openai 缺省）；协议与 baseUrl/模型清单的兼容性提示文案。
- GUI 侧 parseProviderForm/upsertProviderConfig/存档校验同步；i18n 键新增。

### 5.2 A 段：proxy Claude 协议适配（crates/llm-share-proxy）

- 新增 claude_upstream.rs：ClaudeUpstream 实现 Upstream trait。
  - 请求翻译：OpenAI chat completions 体 → Claude /v1/messages 体
    （system 抽取 role=system 消息；messages 过滤 system；max_tokens 必填映射；
    stream:true 恒开；temperature/top_p 尽力映射或省略）。
  - 鉴权：x-api-key + anthropic-version: 2023-06-01，URL 为 {base}/v1/messages
    （base 兼容两种形态：已含 /v1 或裸域名）。
  - 响应翻译：Claude SSE 事件（message_start / content_block_delta.text / message_delta.usage /
    message_stop）→ OpenAI SSE 行（data: {choices:[{delta:{content}}]} / data: {usage...} /
    data: [DONE]），供既有 serve.rs 的 SseSplitter/extract_usage 原样消费。
  - 错误透传：Claude 4xx/5xx 错误体 → UpstreamFailure::Rejected(status)，
    断流 → UpstreamFailure::Broken。
- ModelRoute 增加 protocol 标记（或由装配方选择 Upstream 实现，评审定）；
  ProxyConfig 装配方按 provider.protocol 注入对应 Upstream。
- 测试：进程内 mock Claude 上游，覆盖请求翻译 roundtrip、SSE 翻译（含 usage 抽取）、
  错误映射、baseUrl 形态归一。

### 5.3 B 段：provider 配置持久化 + 出借方常驻 serve

- 新增 ProviderStore（GUI 侧，src-tauri/src/llm_share/provider_store.rs）：
  - 文件 <app-data>/llm-share/providers.json（version=1 信封，tmp+rename 原子写；
    只存 id/name/baseUrl/protocol/models/createdAt + apiKeyRef，不存明文 key）。
  - 密钥文件 <app-data>/llm-share/keys/<providerId>.key，0600 权限
    （复用 llm-share-proxy::keystore 先例）。
  - 命令：provider_list / provider_save（apiKey 明文仅入参，落 0600 文件）/ provider_remove。
- 出借方 serve 装配（src-tauri/src/llm_share/serve.rs）：
  - node_start 时若 offer.json 有效且 providers 匹配则装配 LenderProxy + ServeHandler
    （协议 ID /llm-share/proxy/1 已消费，handler 内从 gate 取入站 peer）。
  - 模型路由：offer.models 中每个模型 → 对应 provider（模型归属 provider 的 models 清单；
    冲突=模型同时属于多个 provider 时按配置序取首个并告警，评审定）。
  - node_stop 卸载；serve 装配失败=显式告警日志，不阻断节点启动
    （可观测信号，非静默吞错）。
- 契约 §16 加法（v12）：provider 三命令 + serve 状态命令 + share 命令（§5.4）。

### 5.4 C 段：分享链接全链

- 链接格式（冻结契约，对齐 ACP 先例）：

      dsh-llm-share://v1?peer=<base58PeerId>&addr=<addr>&token=<32hex>&exp=<unix秒>&sid=<shareId>&models=<model1,model2>

  - addr 可重复（QUIC/TCP/中继多地址）；token=128-bit 随机 hex，原文只在创建响应与链接出现一次；
    exp/sid 是展示性提示，权威判定在出借方；models 为可分享模型清单（兑换时写入 allowlist 的限定模型）。
  - 解析规则：scheme 不符拒绝；peer/token 缺失拒绝；未知参数忽略；models 缺省=不限模型。

- 分享台账 <app-data>/llm-share/shares.json（version=1 信封，tmp+rename 原子写）：
  - ShareEntry{share_id, token_sha256, provider_id, models, max_activations(默认1),
    activations, expires_at_unix, revoked, note, created_at, bound_peer}。
- 命令面（GUI+CLI 对等）：
  - share_create(providerId, models, expiresAt, maxActivations, note) → {link, shareId}
  - share_list / share_revoke(shareId)
  - share_redeem(link)（好友侧）：解析 → 登记 addr → 拨号兑换帧 →
    出借方校验（存在/未撤销/未过期/未超次/未绑定或绑定者=本 peer）→ 写 allowlist
    （share 来源条目，fingerprint=share:<shareId>）→ 返回 offer 快照（models/spare/period）
    供借方向导预填。
- 聊天集成：
  - 出借方：分享面板「生成链接」→ 复制或插入聊天输入框发送；聊天正文识别链接渲染卡片。
  - 好友侧：聊天消息卡片（复用 acp ShareMessageCard 视觉模式，独立域）点击 →
    share_redeem → 成功 toast + 跳转 llm-share 借方向导（targetPeer/model 预填，messages 自填）。

### 5.5 D 段：E2E 与文档

- itest 扩展（crates/p2p-itest）：双节点 + 进程内 mock Claude 上游 +
  share 链接生成→兑换→borrow 全链（验收口径 B1-B7，见 §8）。
- 契约 §16 更新为 v12（命令表 + 语义红线），gui-contract 落 main；
  docs/ops/p2pctl-ai-guide.md 九条目同步（CLI 对等命令）。
- 冒烟文档 docs/ops/llm-share-smoke.md 补充双协议/分享链路步骤。

## 6. 交互流（使用者视角）

    出借方 A（配置了 provider，发布 offer）
      1. 分享面板选 provider → 生成 dsh-llm-share:// 链接（可设一次性/有效期/模型范围）
      2. 链接作为聊天消息发给好友 B（或复制粘贴）
    好友 B
      3. 聊天卡片点击「用链接加入」→ 解析+兑换 → A 侧 allowlist 落 share 条目 → 返回 offer 快照
      4. 借方向导预填 targetPeer=A/model=分享模型 → 输入问题 → borrow
      5. P2P 流：B→A /llm-share/proxy/1 → A 按 provider.protocol 调上游（OpenAI 直发/Claude 翻译）
         → SSE 逐帧转发回 B → 收据+账本（E10 原样）

## 7. 安全与合规（评审重点）

- apiKey：仅出借方本机 0600 落盘；不落 wire、不进日志、不进 offer/台账/链接。
- share token：128-bit 随机，台账只存 sha256；链接原文只在创建响应出现一次。
- 兑换即绑定：首次激活绑定 PeerId；他人复用被拒（审计 share-reuse-denied）。
- allowlist 默认拒绝语义不变；share 来源条目可随 revoke 级联移除。
- 合规红线沿用 E10 §8（不翻墙、不做开放市场）；N1-N5 边界不破。

## 8. 验收口径（机械判定，评审后冻结）

- B1 双协议 roundtrip：openai 直发与 claude 翻译后 SSE 语义一致（itest mock 断言）。
- B2 provider 管理：save/list/remove 命令 + 0600 密钥文件 + 存档损坏显式报错。
- B3 常驻 serve：node_start 后 /llm-share/proxy/1 可被双节点 itest 拨通；
  未发布 offer 或 provider 缺失=显式告警日志且节点照常启动。
- B4 链接全链：share_create → 聊天文本发送 → share_redeem → allowlist 出现 share 条目
  → borrow 成功；一次性链接二次兑换被拒；revoke 后兑换被拒；过期被拒。
- B5 密钥与 token 不落任何 wire/日志（itest 断言 + 日志审计）。
- B6 GUI 门禁：pnpm test/build/check:i18n + cli-parity + ai-docs-sync 全绿。
- B7 Rust 门禁：cargo test --workspace + clippy -D warnings + make check 全绿。

## 9. 任务拆分与并行（评审后定稿）

- W1 proxy Claude 适配（crates/llm-share-proxy/**，域互斥，可与 W2 并行）。
- W2 分享链接 crate（新增 crates/llm-share-link/**：链接编解码+台账+兑换纯逻辑，
  GUI/CLI/itest 三方复用，可与 W1 并行）。
- W3 src-tauri 接线（provider_store + serve 装配 + share/provider/serve 命令；
  依赖 W1+W2 契约面冻结）。
- W4 GUI 视图（provider 表单协议字段 + 分享面板 + 聊天卡片 + i18n/menu.def/App.tsx 登记；
  按 W2/W3 冻结契约 + mock-ipc 并行推进）。
- W5 E2E + 契约文档 + 门禁收尾（依赖 W3+W4）。

每个 W 独立 worktree/分支，协调者机械验收后 ff 合并 main，收尾四步。

