import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

import type {
  ChatFriendJson,
  ChatMediaFile,
  ChatMessageJson,
  ChatSendReport,
  DialReport,
  DiagBackend,
  GroupJson,
  GroupMessageJson,
  GroupSendReport,
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
  UpdateDownloadBackend,
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
  // IM 群聊命令面（im-group-design §7）；可选参数统一传 null（同 chat 段约定）。
  groupCreate: (name, memberIds) =>
    invoke<GroupJson>("group_create", { name, memberIds }),
  groupList: () => invoke<GroupJson[]>("group_list"),
  groupInvite: (groupId, memberIds) =>
    invoke<GroupJson>("group_invite", { groupId, memberIds }),
  groupKick: (groupId, memberId) =>
    invoke<GroupJson>("group_kick", { groupId, memberId }),
  groupLeave: (groupId) => invoke<GroupJson>("group_leave", { groupId }),
  groupRename: (groupId, name) =>
    invoke<GroupJson>("group_rename", { groupId, name }),
  groupDisband: (groupId) => invoke<GroupJson>("group_disband", { groupId }),
  groupSend: (groupId, kind, text, media, replyTo) =>
    invoke<GroupSendReport>("group_send", {
      groupId,
      kind,
      text: text ?? null,
      media: media ?? null,
      replyTo: replyTo ?? null,
    }),
  groupHistory: (groupId, beforeId, limit) =>
    invoke<GroupMessageJson[]>("group_history", {
      groupId,
      beforeId: beforeId ?? null,
      limit: limit ?? null,
    }),
  groupMediaFile: (groupId, messageId) =>
    invoke<ChatMediaFile>("group_media_file", { groupId, messageId }),
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

// 契约 v8 §13 加法（G-U3）：下载安装面。check 拿到的 Update 句柄由本模块持有，
// downloadAndInstall 复用该句柄，避免下载前二次联网；浏览器/dev 无插件环境走 mock。
let pendingUpdate: Update | null = null;

const tauriUpdateDownloadBackend: UpdateDownloadBackend = {
  async checkRemoteUpdate() {
    pendingUpdate = await check();
    if (!pendingUpdate) return null;
    return { version: pendingUpdate.version, notes: pendingUpdate.body ?? null };
  },
  async downloadAndInstallUpdate(onProgress) {
    if (!pendingUpdate) throw new Error("尚未检查到可用更新，无法下载安装");
    let totalBytes: number | null = null;
    let downloadedBytes = 0;
    await pendingUpdate.downloadAndInstall((event) => {
      if (event.event === "Started") {
        totalBytes = event.data.contentLength ?? null;
        downloadedBytes = 0;
      } else if (event.event === "Progress") {
        downloadedBytes += event.data.chunkLength;
      }
      onProgress({ downloadedBytes, totalBytes });
    });
    pendingUpdate = null;
  },
  relaunchApp: () => relaunch(),
};

export const updateDl: UpdateDownloadBackend = mockIpc
  ? (await import("./mock-update")).mockUpdateDownloadBackend
  : tauriUpdateDownloadBackend;
