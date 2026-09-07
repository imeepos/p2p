# llm-share-link 波派单：W3 / W5b（协调者：主会话，2026-09-08）

> 设计基线：docs/design/llm-share-link-design.md v2（已合 main）+ 契约 docs/design/gui-contract.md §16.6（v13，已合 main）。
> 已完成：W0 契约冻结（a7ea914）。在途：W1（proxy Claude 适配）、W2（llm-share-link crate + p2p-cli 共享逻辑 + CLI）、W4（GUI 视图）。
> 合并序：W1 → W2 → W3 → W4 → W5b；协调者机械验收后 ff 合并，收尾四步（push/ff-merge/worktree remove/branch 删）。

## 通用纪律（每个专属会话必读）

- 在主树执行 git worktree add -b <分支> /Users/imeepos/ext512/p2p/.worktrees/<目录> main 创建自己的 worktree；全部改动只在本 worktree，禁碰主树。
- 提交纪律 type(scope): subject；一次提交一个独立变更；配套测试随主变更同提交；禁 git add -A。
- 红线：单文件 ≤300 行（推荐 ≤200）、函数 ≤60 行、失败路径显式报错留可观测信号、非测试代码零 unwrap/expect/panic、不用 emoji、注释不写废话。
- 完成后：worktree 内跑完全部验收命令（全绿）→ git push origin <分支> → 停等协调者机械验收。不自行合并 main。
- 环境事实：cargo 在 ~/.cargo/bin（export PATH）；gtimeout 在 /opt/homebrew/bin；仓库远端是 origin（github）。

## W3 src-tauri 接线（分支 feat/w3-tauri-serve，worktree w3-tauri-serve）

### 需求

1. provider 命令面：llm_share_provider_list / provider_save / provider_remove 三命令（契约 §16.6 表），薄封装 crates/p2p-cli llm_share provider 逻辑（与 CLI 同一事实源，chat.rs 先例）；存档/密钥路径与 CLI 相同（GUI app 数据目录 = LlmShareStore root，密钥 0600）。
2. 分享命令面：llm_share_share_create / share_list / share_revoke 三命令（契约 §16.6 表）；share_revoke 触发 allowlist 按 source 级联（复用 p2p-cli share 逻辑）。
3. serve_redeem 借方编排：llm_share_share_redeem 命令——解析链接（llm-share-link::parse_link）→ 一次性节点拨号（对齐 borrow 既有编排：addr 登记优先、bootstrap 注入）→ /llm-share/redeem/1 请求帧兑换 → 成功后经既有 offer 拉取取回快照 → 返回 LlmShareRedeemResult{offer{peer,models,spare,periodEnds}, shareId, owner}；兑换结构化拒绝码（share-revoked/expired/exhausted/bound-other/invalid）照契约原样透出（业务结果非 Err）；scheme/peer/token 缺失=参数错误显式 Err。
4. 出借方常驻 serve 装配（设计 §5.3）：node_start 时（chat install 之后）若 offer.json 有效且 providers 可匹配则装配出借方代理服务——/llm-share/proxy/1 与 /llm-share/redeem/1 两个 handler，均实现 handle_inbound(peer, stream) 用握手认证 PeerId（禁用 ConnectionGate、禁信帧内自报身份）；装配前查节点已注册协议防静默覆盖；模型→provider 唯一映射（冲突装配失败进 lastError）；AllowlistGate 共享句柄贯通 allow/deny/兑换写入与 admit 判定（文件+内存同源，兑换激活互斥临界区，写序 allowlist 先激活后）；幂等索引持久化（finalize 收据 append 到 llm-share/ledger.json，启动重建 settled 索引防 req_id 双记账）；装配输入=启动快照（运行中 offer/providers 变更不热更，状态可见提示）。
5. serve 状态可查询：RunningNode 持有装配结果（assembled/providerId/models/lastError），llm_share_serve_status 命令读取；装配失败=告警日志+lastError 记录+节点照常启动（不回滚不占槽失败）。
6. allow 命令适配：llm_share_allow 透传 source/expires_at（缺省 None，语义不变）。
7. CLI 对等收口：p2p-cli share 逻辑补 share_redeem 编排 + apps/cli 增 llm-share share redeem 命令 + docs/ops/p2pctl-ai-guide.md 增对应条目（参数名与 clap 逐字一致）；cli-parity.tsv 登记 7 mapped（provider list/save/remove、share create/list/revoke/redeem）+ 1 exempt（llm_share_serve_status，reason：serve 生命周期跟随 GUI 常驻节点，无 CLI 常驻进程面，acp_console_status 先例）——TSV 行与 lib.rs generate_handler 注册同提交落（中间态守卫不红）。

### 边界

- 只动 apps/gui/src-tauri/**、crates/p2p-cli/src/llm_share/**（share redeem 编排与 allowlist 适配若需）、apps/cli/src/llm_share/**、docs/ops/p2pctl-ai-guide.md、cli-parity.tsv、根 src-tauri Cargo.toml path dep。
- 禁碰 apps/gui/src/**（W4 域）、crates/llm-share-proxy/**（W1 域）、crates/llm-share-link/**（W2 域，只作依赖消费）。
- 契约 §16.2 全部红线继续适用（业务拒绝非 Err、密钥/roken 不进日志与返回体、数据文件只经流程写）。

### 验收（机械，全绿才算完成）

1. cd apps/gui/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
2. export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p p2p-cli && cargo clippy -p p2p-cli -- -D warnings
3. make check（worktree 根；含 cli-parity 8 行齐 + ai-docs-sync 含 redeem 条目）
4. 报告：分支名、提交列表（含 message）、测试摘要、遗留问题；push 后停等。

## W5b E2E + 冒烟 + 门禁收尾（分支 feat/w5b-e2e，worktree w5b-e2e；待 W3+W4 合并后派）

### 需求

1. itest 升级：llm_share_common 的 ServeHandler 升 handle_inbound（弃 InboundPeers gate 串线绕行）。
2. 新用例（验收口径设计 §8）：
   - B1 双协议 roundtrip：mock OpenAI 与 mock Claude 两条链路 SSE 语义一致（Claude 侧 usage 必须经 message_start input_tokens + message_delta output_tokens 合成命中 extract_usage，断言收据 usage 与上游一致）。
   - B4 链接全链：share_create → 链接文本 → redeem（认证 PeerId 入 allowlist，source=share:<id>，模型集/到期正确）→ borrow 成功；二次兑换（异 peer）被拒；revoke 后兑换被拒且 allowlist 条目级联移除；过期兑换被拒。
   - B6 双借方并发归属：两借方同时拨号，各自收据/流水归属正确（handle_inbound 无串线）。
   - B7 重启幂等：出借方 serve 重建 settled 索引后同 req_id 重放被 DuplicateReqId 拒。
   - B5 泄露面断言：P2P 帧与上游 mock 收到的 HTTP 请求体不含 apiKey 明文（apiKey 仅允许出现在对上游鉴权头）；share 台账/列表无 token 原文。
3. 冒烟文档：docs/ops/llm-share-smoke.md 补充双协议 provider 配置、分享链接生成→兑换→借用、serve_status 查看步骤。
4. README crate 地图补 llm-share-link 一行。

### 边界

- 只动 crates/p2p-itest/**、docs/ops/llm-share-smoke.md、README.md（crate 地图一行）。
- 不改任何生产代码；发现产品缺陷回报告协调者裁决，不自行扩修。

### 验收（机械，全绿才算完成）

1. export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p p2p-itest --test llm_share_wave（含新用例两连绿）
2. make check 全绿
3. 报告：分支名、提交列表、测试摘要（逐用例）、遗留问题；push 后停等。

