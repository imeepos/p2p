import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  ChatFriendJson,
  ChatMediaFile,
  ChatMessageJson,
  ChatSendReport,
  DialReport,
  DiagBackend,
  GuiConfig,
  IpcBackend,
  MetricsJson,
  MetricsPoint,
  NodeEventJson,
  NodeEventHandler,
  NodeProfile,
  NodeStatus,
  PingOutcome,
  UpdateCheckResult,
} from "./ipc-types";

const NODE_EVENT_CHANNEL = "node-event";

export const useMockIpc = import.meta.env.VITE_MOCK_IPC === "1";

// mock 后端仅在 VITE_MOCK_IPC=1 时动态加载：静态 import 会把 mock 全量打进
// prod bundle（E9-Q0 T4）；顶层 await 使本模块成为异步模块，vite/vitest 均支持。
const mockIpc = useMockIpc ? (await import("./mock-ipc")).mockBackend : null;

const tauriBackend: IpcBackend = {
  nodeStart: (cfg) => invoke<NodeStatus>("node_start", { cfg }),
  nodeStop: () => invoke<NodeStatus>("node_stop"),
  nodeStatus: () => invoke<NodeStatus>("node_status"),
  metricsGet: () => invoke<MetricsJson>("metrics_get"),
  metricsHistory: () => invoke<MetricsPoint[]>("metrics_history"),
  configGet: () => invoke<GuiConfig>("config_get"),
  configSave: (cfg) => invoke<GuiConfig>("config_save", { cfg }),
  peerDial: (target) => invoke<DialReport>("peer_dial", { target }),
  peerConnect: (peerId) => invoke<DialReport>("peer_connect", { peerId }),
  peerDisconnect: (peerId) => invoke<boolean>("peer_disconnect", { peerId }),
  peerPing: (peerId, timeoutMs) =>
    invoke<PingOutcome>("peer_ping", { peerId, timeoutMs }),
  identityReset: (confirm) =>
    invoke<NodeStatus>("identity_reset", { confirm }),
  profileGet: () => invoke<NodeProfile>("profile_get"),
  profileSave: (profile) => invoke<NodeProfile>("profile_save", { profile }),
  updateCheck: () => invoke<UpdateCheckResult>("update_check"),
  updateOpenReleasePage: (url) =>
    invoke<void>("update_open_release_page", { url }),
  // 契约 v7 §12.1：chat 命令面；可选参数统一传 null（serde Option 反序列化 None）。
  chatFriendsList: () => invoke<ChatFriendJson[]>("chat_friends_list"),
  chatFriendAdd: (peerId, nickname, addrs) =>
    invoke<ChatFriendJson>("chat_friend_add", { peerId, nickname, addrs }),
  chatFriendRemove: (peerId) =>
    invoke<boolean>("chat_friend_remove", { peerId }),
  chatFriendUpdate: (peerId, patch) =>
    invoke<ChatFriendJson>("chat_friend_update", {
      peerId,
      group: patch.group ?? null,
      nickname: patch.nickname ?? null,
      note: patch.note ?? null,
    }),
  chatHistory: (peer, beforeId, limit) =>
    invoke<ChatMessageJson[]>("chat_history", {
      peer,
      beforeId: beforeId ?? null,
      limit: limit ?? null,
    }),
  chatSend: (peer, kind, text, media, replyTo) =>
    invoke<ChatSendReport>("chat_send", {
      peer,
      kind,
      text: text ?? null,
      media: media ?? null,
      replyTo: replyTo ?? null,
    }),
  chatMediaFile: (peer, messageId) =>
    invoke<ChatMediaFile>("chat_media_file", { peer, messageId }),
  onNodeEvent: (handler: NodeEventHandler) =>
    listen<NodeEventJson>(NODE_EVENT_CHANNEL, (event) => handler(event.payload)).then(
      (unlisten) => () => {
        unlisten();
      },
    ),
};

// 唯一 IPC 出口：视图层只准经由本对象，禁止直接 import @tauri-apps/api。
export const ipc: IpcBackend = mockIpc ?? tauriBackend;

// 契约 v3 加法（G-H 观测）：诊断命令面，与节点控制面分离；诊断视图只准经由 diag。
// 诊断固定走真实 Tauri IPC（2026-09-03 裁决：诊断页禁止展示 mock 数据）；
// mock-diagnostics 仅测试内 vi.mock 使用，运行时不再按 VITE_MOCK_IPC 切换。
const tauriDiagBackend: DiagBackend = {
  logPath: () => invoke<string>("frontend_log_path"),
  logTail: (maxLines) => invoke<string[]>("frontend_log_tail", { maxLines }),
  logClear: () => invoke<void>("frontend_log_clear"),
};

export const diag: DiagBackend = tauriDiagBackend;
