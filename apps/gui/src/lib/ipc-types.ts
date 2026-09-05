// 与 docs/design/gui-contract.md v1 逐字对齐，禁止私自改名；缺口走协调会话加法修订。

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
  // IM 群聊加法（docs/design/im-group-design.md §7）：入站群消息 / 送达 acks 推进 /
  // roster 变更回执（建群、邀请、移除、退群、解散、改名）。
  | { type: "chat_group_message"; groupId: string; message: GroupMessageJson; tsMs?: number }
  | { type: "chat_group_status"; groupId: string; messageId: string; acks: string[]; status: GroupDeliveryStatus; tsMs?: number }
  | { type: "chat_group_state"; group: GroupJson; tsMs?: number };

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

// 契约 v6 §11 加法：本机节点资料（纯展示，仅存本机，不随发现协议广播）。
export interface NodeProfile {
  name: string; // trim 后 ≤64 字符；空串 = 未命名
  description: string; // ≤280 字符；可空
  avatar: string | null; // data URL（png/jpeg/webp base64，总长 ≤200_000）；null = 未设置
}

// 契约 v7 §12.3 加法（IM 聊天）：与 docs/design/gui-contract.md §12.3 逐字对齐，禁止改名。
export type ChatKind = "text" | "image" | "audio" | "video" | "file";

export type ChatMessageStatus = "pending" | "sent" | "delivered" | "failed";

export interface ChatFriendJson {
  peerId: string; // base58
  nickname: string; // trim 后 ≤64；空串回退 PeerId 缩略
  addrs: string[]; // ip/u端口 = QUIC，ip/t端口 = TCP（对齐 §6 语法）
  note?: string | null;
  group?: string | null; // 分组名（IM-T43）；null/缺省 = 未分组；空串归一化 null，不落盘
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

export interface IpcBackend {
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
  chatFriendAdd(
    peerId: string,
    nickname: string,
    addrs: string[],
  ): Promise<ChatFriendJson>;
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
  onNodeEvent(handler: NodeEventHandler): Promise<UnlistenFn>;
}

// 契约 v3 加法（G-H 观测）：诊断命令面，与节点控制面分离；mock/tauri 同签名。
export interface DiagBackend {
  logPath(): Promise<string>;
  logTail(maxLines: number): Promise<string[]>;
  logClear(): Promise<void>;
}
