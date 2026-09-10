# /llm-share/redeem/1 规范

状态：stable；自 2026-09-07；归属 crates/llm-share-link；符合性：Core=兑换请求/应答帧、结构化拒绝码全表、一次性激活与限时语义、防 TOCTOU 激活；Extended=失败冷却限流（装配方行为）。

## 1. 概览

分享链接兑换协议。出借方生成一次性分享 token（链接形态分发），借方以本协议把 token 兑换为本机准入：出借方校验通过后将借方认证 PeerId 写入 allowlist（条目来源 `source=share:<shareId>`，模型集限定、到期=分享到期），此后借方经 /llm-share/proxy/1 使用额度。兑换本身零记账流水。

`dsh-llm-share://` 链接是分享的展示性凭证，其组装、解析、扫码分发均属应用层，不在本协议帧面内；本页只写兑换帧语义。链接与协议的关系：链接携带 token 原文与 peer/addr/exp/sid/models 提示字段，兑换的权威判定全部在出借方台账，链接字段仅供参考（exp 畸形视为缺省、未知参数必须忽略）。

## 2. 线格式

底座帧封装（wire-format.md）单帧承载，载荷上限 1 MiB（1048576 字节）。开流首帧为协议 ID（`/llm-share/redeem/1` UTF-8），随后一问一答各一帧。

### 2.1 请求帧

```json
{ "token": "32 位小写 hex" }
```

token 形态必须校验：恰 32 字符、全部小写十六进制（128-bit CSPRNG 的 hex 编码）。形态不符按 invalid 拒绝（见 §4）。

### 2.2 应答帧

成功：`{ "ok": true }`，不得携带 code 字段。
失败：`{ "ok": false, "code": "<拒绝码>" }`，code 必须存在且恰一个。

结构化拒绝码全表（wire 形态 kebab-case）：

| code | 语义 |
|---|---|
| invalid | token 无法识别（查无此分享 / token 哈希不匹配 / 请求帧非法 / 服务端激活前置失败） |
| share-revoked | 分享已被出借方撤销 |
| expired | 已过 expires_at（now >= expires_at 即过期） |
| exhausted | 激活次数已满 |
| bound-other | 已绑定其它 PeerId |

## 3. 时序与状态机

### 3.1 兑换事务

1. 借方解析链接取 token（链接解析失败属参数错误，不发帧）；拨号出借方（链接 addr 逐条登记优先，缺省 rendezvous 查号），建议单步超时护栏 30 秒。
2. 借方开流（协议 ID 首帧由开流写出），写请求帧，读应答帧，关流。一条流一问一答，不得在同一流复用。
3. 出借方处理序：读帧；帧非法或处于失败冷却（§4.2）则回 invalid；否则执行激活临界区（§3.3）并回应答。入站身份必须取握手认证 PeerId，帧内自报身份不得采信。

### 3.2 一次性/限时语义（台账判定序，以代码为准）

分享台账条目字段：share_id（UUID）、token_sha256、provider_id、models、max_activations、activations、expires_at_unix、revoked、note、created_at、bound_peer。判定必须按以下次序短路：

1. 台账中无 token_sha256 匹配条目 → invalid；
2. revoked=true → share-revoked；
3. now >= expires_at_unix → expired；
4. bound_peer = 本 PeerId → 幂等成功（不消耗激活次数，重复兑换必须成功且不重复计数）；
5. bound_peer = 其它 PeerId → bound-other；
6. activations >= max_activations → exhausted；
7. 否则激活：activations += 1，bound_peer = 借方 PeerId。

v1 的 max_activations 固定为 1（不做多 peer 各激活一次）。

### 3.3 激活临界区（防 TOCTOU）

读-查-写必须在同一互斥临界区内完成（allowlist 门禁写锁），且保持写序：先落 allowlist 条目，后持久化台账激活。中断时允许「多授」（allowlist 已落而台账未记激活），借方重试经幂等路径收敛，不得出现「台账已耗而准入未落」的单侧少授。台账落盘为 tmp+rename 原子写。

## 4. 错误语义

| 场景 | 应答 | 双方行为 |
|---|---|---|
| token 查无 / 哈希不符 | invalid | 不外泄分享是否存在；借方停止重试 |
| 已撤销 / 过期 / 激活满 / 绑定他人 | 对应拒绝码 | 借方原样透出拒绝码，属业务结果非传输错误 |
| 请求帧非 JSON 或缺 token | invalid | 出借方计一次失败（冷却计数） |
| 台账读失败 / allowlist 写失败 / 台账持久化失败 | invalid | 出借方必须留 ERROR 级日志；share 不消耗（激活未持久化时） |
| 连续失败达冷却阈值 | invalid（冷却期内一律） | 见 §4.2 |
| 应答 ok=false 而 code 缺失，或 ok=true 携带 code | - | 协议违例，借方必须显式报错，不得当作成功或业务拒绝 |

传输层失败（建连失败、超时、流中断）不产生拒绝码；借方报传输错误，可安全重试——同 PeerId 二次兑换在出借方幂等成功，不重复消耗激活。

### 4.2 失败冷却（Extended）

出借方按认证 PeerId 对连续失败计数：连续失败达 3 次（COOLDOWN_THRESHOLD）进入 60 秒（COOLDOWN_SECS）冷却；冷却期内兑换请求一律回 invalid 并留 WARN 日志；成功兑换清零计数。装配方可以选择不启用，协议面不新增拒绝码。

## 5. 安全考量

token 为 128-bit CSPRNG，形态即 32 位小写 hex；台账只存 sha256 摘要，原文只出现在创建响应与链接中各一次，出借方不得留原文。兑换激活绑定握手认证 PeerId，防帧内身份伪造。拒绝码包络统一：invalid 同时覆盖「不存在」与「校验失败」，不外泄分享存在性。临界区串行 + 写序保证并发兑换同一 share 至多一个成功。失败冷却缓解对单 token 的在线枚举。撤销传播：出借方撤销分享时必须级联删除来源为 `share:<shareId>` 的 allowlist 条目（已激活者准入随撤）。

## 6. 兼容与版本

拒绝码只增不改义；借方对未知拒绝码必须按失败处理并原样展示。ok/code 包络不得变更。链接参数集（peer/addr/token/exp/sid/models）可加法扩展，未知参数必须忽略。台账文件信封版本（FORMAT_VERSION=1）为本地存储格式，不属协议面；版本不符必须显式报错，不得静默回空表（默认拒绝语义下静默回空会把既有授权翻转成全拒）。

## 7. 测试向量

无（候选：redeem-frames.json 请求/应答帧与五拒绝码样例）。

## 8. 实现状态与出处

- 帧与拒绝码：crates/llm-share-link/src/redeem.rs（RedeemRequest/RedeemResponse/RedeemCode，PROTOCOL_ID）。
- 台账与激活判定序：crates/llm-share-link/src/ledger.rs（ShareLedger::redeem/ShareEntry/FORMAT_VERSION）。
- token 生成与摘要：crates/llm-share-link/src/token.rs；链接形态校验（应用层）：crates/llm-share-link/src/link.rs。
- 出借方 handler（互斥临界区、写序、冷却）：apps/gui/src-tauri/src/llm_share/serve/redeem.rs；allowlist 门禁与级联删除 apps/gui/src-tauri/src/llm_share/serve/gate.rs；serve 装配 apps/gui/src-tauri/src/llm_share/serve/mod.rs。
- 借方编排（拨号/超时/协议违例判定）：crates/p2p-cli/src/llm_share/share_redeem.rs；分享创建/撤销 crates/p2p-cli/src/llm_share/share.rs（DEFAULT_TTL_SECS=86400，MAX_TTL_SECS=604800）。

漂移登记（代码为准）：

1. 服务端 handler 位于 apps 装配层（GUI serve），crates/llm-share-link 只提供帧与台账纯逻辑；CLI（p2p-itest/测试桩）另有剧本桩实现，两处帧语义一致。
2. 冷却期内与激活前置失败均回 invalid，不设专用拒绝码（统一包络不外泄成因的拍板），成因只进出借方日志。
3. 「allowlist 先落、激活后持久化」写序允许中断后多授，依赖同 peer 重兑幂等收敛；协议页 §3.3 为此语义的规范表述。
4. 链接内 exp/sid 为提示值：sid（shareId）不参与兑换判定（判定以 token 哈希为准），exp 不作为到期权威（以台账 expires_at_unix 为准）。
