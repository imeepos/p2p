# ACP 分享链接（Share Link）：聊天页一键分享 agent 操控权 —— 设计方案 v1

> 状态: v1（拍板记录见 §11） | 日期: 2026-09-06
> 依赖: [acp-over-p2p-design.md](acp-over-p2p-design.md)（桥/握手/策略/续连）、
> [im-chat-design.md](im-chat-design.md)（聊天页宿主，本轮不改其线协议）
> 定位: ACP 波之上的加法应用层特性。不改通信内核，不改 /dsh-acp/1 帧结构。

## 1. 定位与目标

被控者（节点主人/owner）在聊天页把托管 agent 的 ACP 操控权以**临时/一次性链接**
分享出去；拿到链接的好友（guest）导入链接即获得一个受限 ACP endpoint，通过对话
操作 agent 及其授权目录（sandbox 监狱目录或 workspace 授权目录）。

1. owner 侧：聊天页内生成分享（设权限：scope/有效期/激活次数/备注），产出链接，
   可复制或作为聊天文本消息发给好友；ACP 页有分享管理（列表/撤销/续发）。
2. guest 侧：ACP 页「用链接加入」粘贴导入，或聊天消息里的链接渲染成卡片一键加入；
   加入后与既有 ACP 会话同构（聊天/工具时间线/权限按钮/scope 徽章）。
3. 安全模型不回退：默认拒绝、TOFU 绑定、凭据不上 wire、审计可观测，全部沿用。

## 2. 链接格式（冻结契约）

    dsh-acp-share://v1?peer=<base58PeerId>&addr=<addr>&token=<32hex>&exp=<unix秒>&sid=<shareId>

- `addr` 可重复出现（QUIC/TCP/中继多地址），guest 侧逐个登记候选。
- `token` = 128-bit 随机 hex，**原文只出现在创建响应与链接里一次**。
- `exp`/`sid` 是展示性提示；**权威判定在 agent 侧**（过期/撤销/次数以 agent 为准），
  guest 不得因本地时钟误判而拒绝尝试。
- 解析规则：scheme 不符即拒绝并提示；参数缺失 `peer`/`token` 即拒绝；未知参数忽略。

## 3. 数据模型：分享台账（agent 侧）

文件 `<data-dir>/acp-shares.json`（version=1 信封，原子写 tmp+rename，损坏显式报错，
沿 acp-common 策略表先例）：

    ShareEntry {
      share_id: Uuid,          // 展示与管理主键
      token_sha256: String,    // 只存哈希；原文永不落盘/进日志/进审计
      scope: Scope,            // Sandbox | Workspace（Owner 不可经分享产生）
      allow_mcp: Vec<String>,
      ask_route: AskRoute,
      max_activations: u32,    // 默认 1
      activations: u32,        // 已激活次数
      expires_at_unix: u64,    // 创建时算出
      revoked: bool,
      note: String,
      created_at: String,      // RFC3339
      bound_peer: Option<String>, // 首次激活绑定的 PeerId
    }

- **一次性 = 激活次数（默认 1）**，不是连接数：激活即绑定 PeerId 并写入策略表，
  该 peer 此后凭策略表正常连接/重连（token 是否继续持有不影响）。
- 撤销（DELETE /shares/{id} 或 p2pctl）：置 revoked；若已绑定 peer 则**级联删除**
  该 peer 的策略表条目（share 来源的条目），已在线连接由桥按既有断链路径收尾。

## 4. 兑换语义（握手，零协议变更）

复用既有 `ClientHello.token` 字段，不加字段、不 bump 握手版本。/dsh-acp/1 会话编排
的授权判定改为两级瀑布：

1. 策略表命中 → 既有路径（token 忽略，行为与今天完全一致）。
2. 策略表未命中且 token 非空 → 查分享台账：
   - 匹配 + 未撤销 + 未过期 + activations < max + （未绑定 或 绑定者=本 peer）
     → **激活**：activations+1、bound_peer=peer、按 share 写策略表条目
     （fingerprint 记 `share:<share_id>`，granted_at 即时刻）、cwd 监狱按 scope 落位、
     审计 `share-redeemed`，此后照常 ready。
   - token 匹配但绑定者 ≠ 本 peer → 拒绝 + 审计 `share-reuse-denied`。
   - 过期/撤销/超次 → 拒绝 + 审计 `share-expired` / `share-revoked` / `share-exhausted`。
   - 不匹配 → 沿用既有 `token-invalid` 拒绝路径。
3. 其余（无 token 且不在表）→ 既有 `peer-not-allowed`，fail-closed 不变。

deny 的对外错误码不区分撤销与过期细节之外的信息泄露面：沿用既有 denied 帧，
新增错误码进 acp-common 错误码表（带审计键）。

## 5. owner 管理面：acp-agent 本地管理 HTTP

acp-agent 新增本地 admin HTTP：只绑 127.0.0.1，Bearer token 鉴权；token 随机生成，
落 `<data-dir>/acp-admin-token`（0600），启动 stdout JSON 行发布
`{"kind":"ready","admin":{"port":N,"token_file":"..."}}`。CLI：`--admin-port`（默认 0=随机），
`--admin-disabled` 可关。

| 端点 | 语义 |
|---|---|
| POST /shares | 创建：入参 {scope, workspace?, allow_mcp?, ask_route?, ttl_secs, max_activations?, note?}；出参含 share_id + **token 原文** + 链接要素（peer/addr 列表）。scope=workspace 而无可解析工作区 → 创建即 422（workspace-unconfigured=未配任何工作区 / workspace-unknown=定向 id 不存在） |
| GET /shares | 列表（脱敏：无 token 原文/哈希，含 activations/exp/bound_peer/revoke 状态与定向 workspace id） |
| GET /workspaces | 本机工作区清单 [{id,name,dir}]（多工作区分享的 GUI 数据源） |
| DELETE /shares/{share_id} | 撤销（§3 级联语义） |
| GET /shares | 列表（脱敏：无 token 原文/哈希，含 activations/exp/bound_peer/revoke 状态） |
| DELETE /shares/{share_id} | 撤销（§3 级联语义） |

多工作区（2026-09-07 加法）：agent 配置 `workspaces: [{id,name,dir}]` 表；
legacy `--workspace-dir` 等价 id=default 的隐式表项。scope=workspace 的分享可
定向具体工作区（workspace 字段，缺省=默认）；兑换激活后工作区 id 随策略条目
生效，jail 按 id 解析子进程 cwd，未知 id 拒绝 spawn。台账/策略表 v1 文件
向后兼容（追加字段 serde default）。

链接生成要素由 agent 组装：peer=本机 PeerId；addr=对外监听地址（含中继可达地址）。
本机 PeerId 与监听地址的获取沿节点既有自省面，不新增底座接口。

## 6. p2pctl 等价命令（CLI 对等纪律）

    p2pctl acp share create --scope sandbox --ttl-secs 86400 [--max-activations 1]
        [--allow-mcp fs] [--ask-route remote_gui] [--note ...] --data-dir ...
      → stdout JSON：{share_id, token, link}
    p2pctl acp share list   --data-dir ...
    p2pctl acp share revoke <share_id> --data-dir ...

与 GUI 管理面同源（都打 admin HTTP 或直读台账，实现自选但语义一致）；
scripts/check/cli-parity.sh 口径覆盖。

## 7. guest 接入：acp-console 按链接直拨

- CLI：`--share-link <url>`；status HTTP：`POST /connect-share` {`"link"`}（GUI 入口）。
- 行为：解析链接（§2）→ 登记 peer 地址候选（沿 --peer PEER@ADDR 同机制）→ 拨号 →
  握手 `token`=链接 token → ready 后与普通 endpoint 无异（本地 WS 哑泵、status/discovery
  可见、reattach 票据照常落盘）。
- 失败可观测：denied 码/reason 进 status 端点与 stdout JSON 行，禁止静默。
- 同一链接重复导入 = 对同 peer 重复连接（激活已在首连完成，之后走策略表），幂等。

## 8. GUI 契约（不动 IM 线协议）

owner 侧：
- 聊天页会话工具条新增「分享 ACP」：弹层设 scope（sandbox 默认/workspace）、有效期
  档位（1h/24h/7d）、激活次数（默认 1）、备注 → 调 agent admin POST /shares →
  展示链接 + 复制 + 「发送到当前聊天」（作为普通文本消息插入，链接即消息正文）。
- ACP 页 owner 分享管理卡：admin GET /shares 列表（状态徽章：有效/已用尽/已过期/
  已撤销/已绑定）+ 撤销按钮 + 与聊天页同源的新建入口。
- agent admin 端点（URL+token）在 ACP 页既有 endpoint 管理处登记（endpoint-storage 加法）。

guest 侧：
- ACP 页「用链接加入」：粘贴 → console POST /connect-share → 成功后连接目录出现
  该 endpoint（scope 徽章=分享 scope）；失败展示 denied 原因。
- 聊天消息正文中含 `dsh-acp-share://` 链接 → 渲染为「加入共享的 ACP」卡片（transcript
  渲染层识别，纯展示，不改 ChatEnvelope/线协议），点击走同一导入路径。

i18n：新增文案全量走 i18n 注册（append-only），pnpm check:i18n 过闸。

## 9. 文件域划分（并行任务互斥，撞域即协调）

| 任务 | 独占文件域 |
|---|---|
| AS1 后端 | apps/acp-common/src/**（share 模型/错误码）、apps/acp-agent/src/**（台账/兑换/管理 HTTP/审计）、apps/cli/src/acp/**（share 子命令） |
| AS2 console | apps/acp-console/src/**（--share-link、/connect-share、票据联动） |
| AS3 GUI | apps/gui/src/**（聊天页/ACP 页/transcript 渲染/i18n/endpoint 存储；中央登记文件仅本任务触碰） |
| AS4 收官 | crates/p2p-itest/**（share 波次 e2e）、apps/acp-agent/tests/ 新增 e2e、docs/ops/acp-guide.md、README.md |

账本（.devloop/loop-state.json）与设计文档归协调会话所有，任务会话不得改写。

## 10. 审计与可观测

- 新审计事件：`share-redeemed / share-reuse-denied / share-expired / share-revoked /
  share-exhausted`（只记 PeerId/share_id/错误码/时间，不含 token 原文与策略细节）。
- token 原文永不落日志：admin HTTP 访问日志、审计、stderr 滚动日志三处同红线。
- agent 台账操作（创建/撤销）在 stdout JSON 行输出一行事件（owner 本机可观测）。

## 11. 拍板记录（2026-09-06，项目负责人）

| # | 问题 | 拍板 |
|---|---|---|
| Q1 | 「一次性」的语义 | 激活次数（默认 1）；绑定 peer 后其连接/重连不受限 |
| Q2 | token 如何上 wire | 复用 ClientHello.token 字段，零协议变更、不 bump 版本 |
| Q3 | owner GUI 创建分享的通道 | agent 本地 admin HTTP（127.0.0.1+token）；不做 GUI→CLI spawn |
| Q4 | 撤销的效力 | 台账 revoked + 级联删除 share 来源的策略条目 |
| Q5 | scope=workspace 的分享前提 | agent 已配 --workspace-dir，否则创建即拒（fail-closed 前移） |
| Q6 | 链接进聊天消息的载体 | 普通文本消息 + transcript 渲染层识别成卡片；不改 IM 冻结契约 |
| Q7 | owner 手填 admin token 的消除（2026-09-07） | acp-agent 启动写本机自描述 ~/.dsh/acp/local-agent.json（0600），GUI 无登记管理端点时自动读入兜底；远端 agent 仍手动登记。同用户 0600 与既有 token 文件同一信任域，token 不进日志/审计红线不变 |