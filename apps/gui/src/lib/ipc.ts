import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
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
const mockDiag = useMockIpc
  ? (await import("./mock-diagnostics")).mockDiagBackend
  : null;

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
const tauriDiagBackend: DiagBackend = {
  logPath: () => invoke<string>("frontend_log_path"),
  logTail: (maxLines) => invoke<string[]>("frontend_log_tail", { maxLines }),
};

export const diag: DiagBackend = mockDiag ?? tauriDiagBackend;
