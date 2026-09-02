import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { mockBackend } from "./mock-ipc";
import type {
  DialReport,
  GuiConfig,
  IpcBackend,
  MetricsJson,
  NodeEventJson,
  NodeEventHandler,
  NodeStatus,
  PingOutcome,
} from "./ipc-types";

const NODE_EVENT_CHANNEL = "node-event";

export const useMockIpc = import.meta.env.VITE_MOCK_IPC === "1";

const tauriBackend: IpcBackend = {
  nodeStart: (cfg) => invoke<NodeStatus>("node_start", { cfg }),
  nodeStop: () => invoke<NodeStatus>("node_stop"),
  nodeStatus: () => invoke<NodeStatus>("node_status"),
  metricsGet: () => invoke<MetricsJson>("metrics_get"),
  configGet: () => invoke<GuiConfig>("config_get"),
  configSave: (cfg) => invoke<GuiConfig>("config_save", { cfg }),
  peerDial: (target) => invoke<DialReport>("peer_dial", { target }),
  peerPing: (peerId, timeoutMs) =>
    invoke<PingOutcome>("peer_ping", { peerId, timeoutMs }),
  identityReset: (confirm) =>
    invoke<NodeStatus>("identity_reset", { confirm }),
  onNodeEvent: (handler: NodeEventHandler) =>
    listen<NodeEventJson>(NODE_EVENT_CHANNEL, (event) => handler(event.payload)).then(
      (unlisten) => () => {
        unlisten();
      },
    ),
};

// 唯一 IPC 出口：视图层只准经由本对象，禁止直接 import @tauri-apps/api。
export const ipc: IpcBackend = useMockIpc ? mockBackend : tauriBackend;
