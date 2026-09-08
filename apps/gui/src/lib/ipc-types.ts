// 与 docs/design/gui-contract.md v1 逐字对齐，禁止私自改名；缺口走协调会话加法修订。

// 契约 §16.6 v13（llm-share-link 波）：双协议 provider + 分享链接 8 命令面 DTO 共用同一
// 定义（lib/llm-share-v13-types.ts），ipc 与视图接缝同时 re-export 防漂移。
import type {
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "./llm-share-v13-types";
export type {
  LlmProviderProtocol,
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRejectCode,
  LlmShareRedeemResult,
} from "./llm-share-v13-types";

export interface GuiConfig {
  quicPort: number; // 0 = 随机
  tcpPort: number; // 0 = 随机
  enableMdns: boolean;
  dataDir: string;
  bootstrap: string[];
  relayAddrs: string[];
  advertisedAddrs: string[];
  observationPort: number | null;
  observationAddrs: string[];
  // 契约 v11 §16.5 加法：仅监听局域网发现；serde default，缺省 false（双向兼容）。
  lanOnly?: boolean;
}

export interface NodeStatus {
  running: boolean;
  peerId: string | null; // base58(sha256(pubkey))
  listenAddrs: string[];
  uptimeSecs: number;
  startedAtMs: number | null;
  config: GuiConfig; // 运行中节点的生效配置；未运行回持久化配置
}

export interface MetricsJson {
  dialDirectOk: number;
  dialDirectFail: number;
  dialPunchOk: number;
  dialPunchFail: number;
  dialRelayOk: number;
  dialRelayFail: number;
  addrDialFailures: number;
  relayReconnects: number;
  gateDenialsTotal: number;
  activeConnections: number;
  relaySessionsActive: number;
}

export type DialHopKind = "direct" | "punch" | "relay";

// 契约 v5 加法：peer_discovered.source，地址簿聚合来源（mdns > rendezvous > manual）。
export type PeerSource = "mdns" | "rendezvous" | "manual";

export interface DialHopJson {
  hop: DialHopKind;
  ok: boolean;
  detail: string;
}

export interface DialReport {
  peer: string;
  hops: DialHopJson[];
  ok: boolean;
  totalMs: number;
}

export interface PingOutcome {
  ok: boolean;
  rttMs: number | null;
  hops: DialHopJson[];
  error: string | null;
}

// 契约 v2：后端每 5s 采样，最近 120 点（10 分钟窗口）。
export interface MetricsPoint {
  tMs: number;
  activeConnections: number;
  relaySessionsActive: number;
  dialOkTotal: number;
  dialFailTotal: number;
}

// 契约 §2 修订：全变体可带可选 tsMs（后端 emit 出口统一盖发射时刻戳，缺省字段不出现在载荷）。
export type NodeEventJson =
  | { type: "peer_discovered"; peer: string; addrs: string[]; source: PeerSource; tsMs?: number }
  | { type: "peer_connected"; peer: string; tsMs?: number }
  | { type: "peer_disconnected"; peer: string; tsMs?: number }
  | { type: "listen_failed"; addr: string; reason: string; tsMs?: number }
  | { type: "dial_failed"; peer: string | null; reason: string; tsMs?: number }
  | { type: "protocol_violation"; peer: string; reason: string; tsMs?: number }
  | { type: "dial_hop"; peer: string; hop: DialHopKind; ok: boolean; detail: string; tsMs?: number }
  | { type: "node_started"; listenAddrs: string[]; tsMs?: number }
  | { type: "node_stopped"; tsMs?: number }
  | { type: "node_error"; reason: string; tsMs?: number }
  // 契约 v7 §12.2 加法：入站新消息（已落盘）与本地发送状态推进。
  | { type: "chat_message"; peer: string; message: ChatMessageJson; tsMs?: number }
  | { type: "chat_status"; peer: string; messageId: string; status: ChatMessageStatus; tsMs?: number }
  | { type: "chat_invite"; peer: string; state: InviteStateJson; tsMs?: number }
  // IM 群聊加法（docs/design/im-group-design.md §7）：入站群消息 / 送达 acks 推进 /
  // roster 变更回执（建群、邀请、移除、退群、解散、改名）。
  | { type: "chat_group_message"; groupId: string; message: GroupMessageJson; tsMs?: number }
  | { type: "chat_group_status"; groupId: string; messageId: string; acks: string[]; status: GroupDeliveryStatus; tsMs?: number }
  | { type: "chat_group_state"; group: GroupJson; tsMs?: number }
  // IMC3 加法（冻结契约）：入群邀请生命周期事件，载荷=邀请条目。
  | { type: "chat_group_invite"; invite: GroupInviteJson; tsMs?: number };

export type NodeEventType = NodeEventJson["type"];

export type NodeEventHandler = (event: NodeEventJson) => void;
export type UnlistenFn = () => void;

// 契约 v4 §9 加法（G-U1/G-U2）：与 docs/design/gui-contract.md 逐字对齐，禁止改名。
export interface UpdateCheckResult {
  currentVersion: string; // 应用当前版本（tauri.conf version）
  latestVersion: string | null; // 无候选时 null
  hasUpdate: boolean;
  releaseUrl: string | null; // release html_url
  releaseName: string | null;
  releaseNotesMd: string | null; // release body 原文
  publishedAtMs: number | null;
  checkedAtMs: number;
}

// 契约 v8 §13 加法（G-U3）：updater 下载安装面（官方 tauri-plugin-updater，minisign 校验）。
export interface RemoteUpdate {
  version: string; // 远端新版本号
  notes: string | null; // 更新清单 notes（当前清单不含，保留扩展位）
}

export interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null; // Started 事件可能缺 contentLength，此时展示不定进度
}

// 与节点控制面（ipc）/诊断面（diag）并列的第三命令面；mock/tauri 同签名。
export interface UpdateDownloadBackend {
  // null = 已是最新；返回非 null 后才允许 downloadAndInstallUpdate
  checkRemoteUpdate(): Promise<RemoteUpdate | null>;
  // 下载并自动安装（macOS 替换 bundle / Windows NSIS 静默安装 / Linux 替换 AppImage）；
  // onProgress 按块回调，完成（含安装）后 resolve
  downloadAndInstallUpdate(
    onProgress: (p: UpdateDownloadProgress) => void,
  ): Promise<void>;
  relaunchApp(): Promise<void>;
}

// 媒体导出面（契约 §12.5 加法，2026-09-07）：保存对话框 + 后台拷贝进度。
export interface MediaExportProgressPayload {
  receivedBytes: number;
  totalBytes: number;
}

export interface MediaExportResult {
  destPath: string;
  totalBytes: number;
}

export interface MediaExportBackend {
  // 系统保存对话框；用户取消返回 null
  pickSavePath(defaultFileName: string): Promise<string | null>;
  // 后台导出（目标路径已选定）；onProgress 流式回报，resolve 即拷贝完成
  exportMedia(
    sourceUrl: string,
    destPath: string,
    onProgress: (p: MediaExportProgressPayload) => void,
  ): Promise<MediaExportResult>;
}

// 契约 v6 §11 加法：本机节点资料（纯展示，仅存本机，不随发现协议广播）。
export interface NodeProfile {
  name: string; // trim 后 ≤64 字符；空串 = 未命名
  description: string; // ≤280 字符；可空
  avatar: string | null; // data URL（png/jpeg/webp base64，总长 ≤200_000）；null = 未设置
}

// 契约 v7 §12.3 加法（IM 聊天）：与 docs/design/gui-contract.md §12.3 逐字对齐，禁止改名。
// IMC3 加法：groupInvite 为 1:1 入群邀请卡片消息专用 kind（协调会话冻结契约）。
export type ChatKind = "text" | "image" | "audio" | "video" | "file" | "groupInvite";

export type ChatMessageStatus = "pending" | "sent" | "delivered" | "failed";

export interface ChatFriendJson {
  peerId: string; // base58
  nickname: string; // trim 后 ≤64；空串回退 PeerId 缩略
  addrs: string[]; // ip/u端口 = QUIC，ip/t端口 = TCP（对齐 §6 语法）
  note?: string | null;
  group?: string | null; // 分组名（IM-T43）；null/缺省 = 未分组；空串归一化 null，不落盘
}

// 邀请制加好友（契约 v9 §12.4）：out=本机待对方同意 / in=对方待本机处理。
export type InviteDirectionJson = "out" | "in";
export type InviteStateJson = "incoming" | "accepted" | "rejected";

export interface FriendInviteJson {
  peerId: string;
  nickname: string;
  addrs: string[];
  note?: string | null;
  direction: InviteDirectionJson;
  tsMs: number;
  delivered: boolean;
}

export interface InviteReportJson {
  invite: FriendInviteJson;
  delivered: boolean;
}

export interface ChatMediaInput {
  name: string; // 原始文件名（展示用，落盘时 sanitize）
  mime: string; // 小写；按 kind 白名单校验（设计 §5），不匹配 Err
  dataBase64: string; // 原始字节 base64（解码后 ≤64MiB，超限 Err）
}

export interface ChatMediaJson {
  name: string;
  mime: string;
  size: number; // 原始字节数
  path?: string | null; // 本端落盘绝对路径（仅返回给本端消费）
}

// IMC3 加法：kind=groupInvite 的 1:1 消息体载荷（卡片渲染输入）。
export interface ChatGroupInviteBody {
  groupId: string;
  groupName: string;
  inviterNickname: string;
  note: string | null;
}

export interface ChatMessageJson {
  id: string; // UUID（发端生成）
  peer: string;
  sender: "me" | "them";
  kind: ChatKind;
  tsMs: number;
  text?: string | null;
  media?: ChatMediaJson | null;
  status: ChatMessageStatus; // 本地状态字段，不跨网
  replyTo?: string | null; // 被引用消息的本端消息 id；null/缺省=无引用（IM-T46A 加法，不校验存在性）
  groupInvite?: ChatGroupInviteBody | null; // 仅 kind=groupInvite 携带（IMC3）
}

export interface ChatSendReport {
  message: ChatMessageJson; // status=delivered=已实时送达；否则 pending（outbox 等待）
  delivered: boolean;
  flushedOutbox?: number; // 本轮命令顺手补投的历史积压条目数；0/缺省=无补投（CLI 演练加法）
}

// chat_media_file 返回：附件落盘绝对路径（仅本端展示用，不跨网）。
export interface ChatMediaFile {
  path: string;
  mime: string;
  name: string;
}

// IM 群聊（docs/design/im-group-design.md §7 逐字对齐，禁止改名）。
export type GroupChatState = "active" | "left" | "kicked" | "disbanded";

// chat_group_status.status：群消息状态机不含 sent（设计 §4：枚举保留不占用）。
export type GroupDeliveryStatus = "pending" | "delivered" | "failed";

export interface GroupJson {
  groupId: string; // UUID（owner 生成）
  name: string; // trim 后 1..=64 字符
  owner: string; // PeerId；名单唯一权威
  members: string[]; // PeerId[]，含 owner，≤32
  rev: number; // roster 版本，仅 owner 单调递增
  state: GroupChatState; // 退群/被踢/解散不删数据，仅置位
  tsMs: number;
}

export interface GroupMessageJson {
  id: string; // UUID（发端生成）
  groupId: string;
  senderId: string; // 作者 PeerId；本端消息判定 senderId === 本机 PeerId
  kind: ChatKind;
  tsMs: number;
  text?: string | null;
  media?: ChatMediaJson | null; // 复用 §12.3 ChatMediaJson
  status: ChatMessageStatus; // sent 不出现（设计 §4 状态机）
  acks: string[]; // 已确认成员 PeerId（仅本端发出的消息非空）
  replyTo?: string | null; // 同 1:1 语义，不校验被引用消息存在性
}

export interface GroupSendReport {
  message: GroupMessageJson;
  acked: number; // 本轮已确认成员数
  recipients: number; // 目标成员数（n-1）
  delivered: boolean; // acked === recipients
}

// IMC3 同意制入群邀请（协调会话冻结契约，与 IMC1 后端逐字对齐，禁止改名）。
// direction 为本端视角：in=待本机处理 / out=本机已发出。
export type GroupInviteDirection = "in" | "out";
export type GroupInviteState = "pending" | "accepted" | "rejected";

export interface GroupInviteJson {
  id: string; // 邀请唯一 id（UUID）
  groupId: string; // 目标群 UUID
  groupName: string; // 快照群名（弹框与卡片直接展示）
  owner: string; // 群主 PeerId
  inviter: string; // 邀请人 PeerId
  invitee: string; // 受邀人 PeerId
  note: string | null; // 邀请备注，可空
  direction: GroupInviteDirection;
  state: GroupInviteState;
  tsMs: number;
  delivered: boolean;
}

// 契约 v10 §15 加法（UX3）：acp-console 托管状态（GUI 壳伴生进程面），逐字对齐冻结契约。
export type AcpConsolePhase =
  | "starting"
  | "ready"
  | "restarting"
  | "failed"
  | "unavailable"
  | "stopped";

export interface AcpConsoleStatus {
  phase: AcpConsolePhase;
  wsUrl?: string; // ready 后：ws://127.0.0.1:<port>
  token?: string; // ready 后：console WS 鉴权 token
  statusUrl?: string; // ready 后：console status HTTP 地址
  adminUrl?: string; // ready 后：agent admin HTTP 地址（ready 行携带才填）
  restarts: number; // 已自动重启次数
  lastError?: string; // 最近一次失败原因（可读中文）
}

export type AcpConsoleEventHandler = (status: AcpConsoleStatus) => void;

// 2026-09-07 裁决加法：本机 agent 自描述（acp-agent 写 ~/.dsh/acp/local-agent.json，
// src-tauri acp_local_descriptor 读出）。分享流据此免手填 admin token；
// null = 本机无描述文件（agent 未跑/远端场景），回落手动登记。
export interface AcpLocalDescriptor {
  adminUrl: string; // http://127.0.0.1:<port>
  token: string;
  peer: string;
  agentName: string;
  writtenAtUnix: number;
}

// ── 契约 v11 §16 加法（LSG 波）：llm-share GUI 面，与 §16.1 逐字对齐，禁止改名 ──

// offer show status 五态：expired/not_yet_valid=常态中性；peer_mismatch/bad_signature=警示。
export type LlmOfferStatus =
  | "live"
  | "expired"
  | "not_yet_valid"
  | "peer_mismatch"
  | "bad_signature";

// 声明视图：声明本体 + remaining_secs/status/file；file 只读展示，GUI 禁直写 offer.json。
export interface LlmOfferView {
  peer: string; // 出借方 PeerId
  models: string[];
  spare: Record<string, number>; // model -> 闲量 token，覆盖全部 models 且 >0
  periodEnds: string; // YYYY-MM-DD 账期截止日
  maxPerReq: Record<string, number> | null; // model -> 单请求上限；null=未显式设限
  rateLimit: { rpm: number; concurrency: number };
  ttlSecs: number;
  retention: string; // 数据留存自述
  issuedAt: number; // 签发 epoch 秒
  expiresAt: number; // 过期 epoch 秒
  remainingSecs: number; // <=0 即过期（常态非错误）
  status: LlmOfferStatus;
  file: string;
}

// offer publish 入参：必填集 IPC 层校验（models>=1、spare 覆盖全部 model 且 N>0、
// period-ends 为 YYYY-MM-DD），与 CLI --model/--spare/--period-ends 同规则。
export interface LlmOfferPublishInput {
  models: string[];
  spare: Record<string, number>;
  periodEnds: string;
  maxPerReq?: Record<string, number> | null;
  rpm?: number; // 缺省 10
  concurrency?: number; // 缺省 2
  ttlSecs?: number; // 缺省 3600
  retention?: string; // 缺省 "none"
}

// allowlist 条目：models null = 不限模型（CLI --model 缺省原话）。
export interface LlmAllowEntry {
  peerId: string;
  models: string[] | null;
  note: string | null;
  grantedAt: string; // ISO8601
}

// allow/deny 操作回执：deny 不存在条目 = ok:false 显式报错，非错误态（§16.1）。
export interface LlmAllowOpReport {
  op: "allow" | "deny";
  peerId: string;
  ok: boolean;
  created: boolean; // allow upsert：新建 true / 刷新 false；deny 恒 false
  message: string;
}

// allow/deny 返回操作后的 allowlist 全量快照；纯读取语义 lastOp 为 null。
export interface LlmAllowlistView {
  entries: LlmAllowEntry[];
  lastOp: LlmAllowOpReport | null;
}

export interface LlmBorrowMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

// borrow 入参：maxTokens/targetPeer 必填（真实成本动作，§16.2.6）；targetPeer 无缺省
// 路径，缺出借方 IPC 层显式报错；reqId 客户端生成 UUID 重试复用，缺省 IPC 层生成。
export interface LlmBorrowRequest {
  model: string;
  messages: LlmBorrowMessage[];
  maxTokens: number;
  targetPeer: string;
  reqId?: string;
}

export type LlmBorrowStatus = "done" | "stream_broken" | "rejected";

// 拒绝码四值原样透出不本地化改写（§16.2.1）；rejected 是业务结果非命令 Err。
export type LlmBorrowRejectionCode =
  | "not_allowlisted"
  | "model_not_served"
  | "freeze_insufficient"
  | "concurrency_exceeded";

export interface LlmBorrowReceipt {
  reqId: string;
  appended: boolean; // req_id 幂等：重试复用同 reqId 不双记（§16.2.3）
  estimated: boolean; // stream_broken 时 true（估算账单）
  disputeWindowSecs: number; // stream_broken 时 259200（72h 争议窗）
}

export interface LlmBorrowUsage {
  input: number;
  output: number;
}

export interface LlmBorrowReport {
  status: LlmBorrowStatus;
  receipt: LlmBorrowReceipt;
  sseCount: number; // sse 原文只出计数，正文不透传（§16.2.6）
  usage?: LlmBorrowUsage | null;
  code?: LlmBorrowRejectionCode | null;
  message?: string | null;
}

export interface LlmLedgerFilter {
  lender?: string | null;
  borrower?: string | null;
  period?: string | null; // 如 "2026-09"
}

// 账本流水条目（append-only 存储序；GUI 只读展示，禁直写 ledger.json）。
export interface LlmLedgerEntry {
  reqId: string;
  period: string;
  lender: string;
  borrower: string;
  model: string;
  input: number;
  output: number;
  tokens: number;
  estimated: boolean;
  ts: number; // epoch 秒
}

export type LlmBalanceDirection = "lent_out" | "borrowed";

// 净差按 lender+period 切分（§16.1）；netAmount 正=出借，负=借入。
export interface LlmBalanceGroup {
  lender: string;
  period: string;
  netAmount: number;
  direction: LlmBalanceDirection;
}

export type LlmReceiptVerdict = "PASS" | "FAIL";

// receipt verify 结果（ai-guide receipt verify --json 同字段，camelCase）。
export interface LlmReceiptVerifyResult {
  verdict: LlmReceiptVerdict;
  reason: string;
  reqId: string;
  period: string;
  lender: string;
  borrower: string;
  model: string;
  input: number;
  output: number;
  estimated: boolean;
  ts: number;
}

export interface IpcBackend {
  acpConsoleStatus(): Promise<AcpConsoleStatus>;
  acpLocalDescriptor(): Promise<AcpLocalDescriptor | null>;
  onAcpConsoleEvent(handler: AcpConsoleEventHandler): Promise<UnlistenFn>;
  nodeStart(cfg: GuiConfig): Promise<NodeStatus>;
  nodeStop(): Promise<NodeStatus>;
  nodeStatus(): Promise<NodeStatus>;
  metricsGet(): Promise<MetricsJson>;
  metricsHistory(): Promise<MetricsPoint[]>;
  configGet(): Promise<GuiConfig>;
  configSave(cfg: GuiConfig): Promise<GuiConfig>;
  peerDial(target: string): Promise<DialReport>;
  peerConnect(peerId: string): Promise<DialReport>;
  peerDisconnect(peerId: string): Promise<boolean>;
  peerPing(peerId: string, timeoutMs: number): Promise<PingOutcome>;
  identityReset(confirm: boolean): Promise<NodeStatus>;
  profileGet(): Promise<NodeProfile>;
  profileSave(profile: NodeProfile): Promise<NodeProfile>;
  updateCheck(): Promise<UpdateCheckResult>;
  updateOpenReleasePage(url: string): Promise<void>;
  chatFriendsList(): Promise<ChatFriendJson[]>;
  chatFriendInvite(
    peerId: string,
    nickname: string,
    addrs: string[],
  ): Promise<InviteReportJson>;
  chatInvitesList(): Promise<FriendInviteJson[]>;
  chatInviteAccept(
    peerId: string,
    nickname: string,
  ): Promise<ChatFriendJson>;
  chatInviteReject(peerId: string): Promise<void>;
  chatInviteCancel(peerId: string): Promise<boolean>;
  chatFriendRemove(peerId: string): Promise<boolean>;
  // IM-T43：好友资料补丁（group/nickname/note 至少一项；addrs 不可经此修改）；
  // 空串 group = 移出分组；peer 不在簿或越界组名 → 可读 Err。
  chatFriendUpdate(
    peerId: string,
    patch: { group?: string | null; nickname?: string | null; note?: string | null },
  ): Promise<ChatFriendJson>;
  chatHistory(
    peer: string,
    beforeId?: string | null,
    limit?: number,
  ): Promise<ChatMessageJson[]>;
  chatSend(
    peer: string,
    kind: ChatKind,
    text?: string,
    media?: ChatMediaInput,
    replyTo?: string | null,
  ): Promise<ChatSendReport>;
  chatMediaFile(peer: string, messageId: string): Promise<ChatMediaFile>;
  // IMC3 入群邀请命令面（冻结契约：invoke 名逐字 snake_case；可选参数统一传 null）。
  chatGroupInvitesList(): Promise<GroupInviteJson[]>;
  chatGroupInviteSend(
    groupId: string,
    peerId: string,
    note: string | null,
  ): Promise<GroupInviteJson>;
  chatGroupInviteAccept(inviteId: string): Promise<void>;
  chatGroupInviteReject(inviteId: string, reason: string | null): Promise<void>;
  // IM 群聊命令面（im-group-design §7；mock 与 tauri 同签名，可选参数统一传 null）。
  groupCreate(name: string, memberIds: string[]): Promise<GroupJson>;
  groupList(): Promise<GroupJson[]>;
  groupInvite(groupId: string, memberIds: string[]): Promise<GroupJson>;
  groupKick(groupId: string, memberId: string): Promise<GroupJson>;
  groupLeave(groupId: string): Promise<GroupJson>;
  groupRename(groupId: string, name: string): Promise<GroupJson>;
  groupDisband(groupId: string): Promise<GroupJson>;
  groupSend(
    groupId: string,
    kind: ChatKind,
    text?: string,
    media?: ChatMediaInput,
    replyTo?: string | null,
  ): Promise<GroupSendReport>;
  groupHistory(
    groupId: string,
    beforeId?: string | null,
    limit?: number,
  ): Promise<GroupMessageJson[]>;
  groupMediaFile(groupId: string, messageId: string): Promise<ChatMediaFile>;
  // 契约 v11 §16 加法（LSG 波）：llm-share 命令面（invoke 名逐字 snake_case，
  // 可选参数统一传 null，同 chat 段约定）。
  llmShareOfferPublish(offer: LlmOfferPublishInput): Promise<LlmOfferView>;
  llmShareOfferShow(): Promise<LlmOfferView>;
  llmShareAllowList(): Promise<{ entries: LlmAllowEntry[] }>;
  llmShareAllow(
    peerId: string,
    models?: string[],
    note?: string,
  ): Promise<LlmAllowlistView>;
  llmShareDeny(peerId: string): Promise<LlmAllowlistView>;
  llmShareBorrow(req: LlmBorrowRequest): Promise<LlmBorrowReport>;
  llmShareLedgerList(filter?: LlmLedgerFilter): Promise<LlmLedgerEntry[]>;
  llmShareLedgerBalance(): Promise<LlmBalanceGroup[]>;
  llmShareReceiptVerify(
    reqId: string,
    lenderPubkey?: string,
  ): Promise<LlmReceiptVerifyResult>;
  // 契约 §16.6 v13 加法：双协议 provider + 分享链接 8 命令面（invoke 名逐字
  // snake_case；apiKey 明文仅入参，providerList 只回掩码；token 只在创建响应出现一次）。
  llmShareProviderList(): Promise<{ providers: LlmProviderView[] }>;
  llmShareProviderSave(config: LlmProviderSaveReq): Promise<LlmProviderView>;
  llmShareProviderRemove(providerId: string): Promise<{ removed: true }>;
  llmShareShareCreate(req: LlmShareCreateReq): Promise<LlmShareCreateResult>;
  llmShareShareList(): Promise<{ shares: LlmShareEntry[] }>;
  llmShareShareRevoke(shareId: string): Promise<{ revoked: true }>;
  llmShareShareRedeem(link: string): Promise<LlmShareRedeemResult>;
  llmShareServeStatus(): Promise<LlmServeStatus>;
  onNodeEvent(handler: NodeEventHandler): Promise<UnlistenFn>;
}

// 契约 v3 加法（G-H 观测）：诊断命令面，与节点控制面分离；mock/tauri 同签名。
export interface DiagBackend {
  logPath(): Promise<string>;
  logTail(maxLines: number): Promise<string[]>;
  logClear(): Promise<void>;
}
