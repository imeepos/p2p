// peers 页 descriptor：动作与邻居表工具栏/行操作同源（useNodeStore → IPC）。
// state() 输出与 peers 表格同口径的行快照（复用 selectPeerList 的可见性过滤）。
import { useNodeStore, selectPeerList } from "@/stores/node-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "peers",
  description: "邻居页：拨号、连接管理与探活",
  actions: [
    {
      name: "dial",
      description: "按拨号目标（multiaddr 或 host:port）发起拨号",
      args: [
        { name: "target", type: "string", required: true, description: "拨号目标" },
      ],
    },
    {
      name: "connect",
      description: "按 PeerId 连接已发现的对端",
      args: [
        { name: "peerId", type: "string", required: true, description: "对端 PeerId" },
      ],
    },
    {
      name: "disconnect",
      description: "断开与对端的连接",
      args: [
        { name: "peerId", type: "string", required: true, description: "对端 PeerId" },
      ],
    },
    {
      name: "ping",
      description: "探活对端并返回 RTT 与路径报告",
      args: [
        { name: "peerId", type: "string", required: true, description: "对端 PeerId" },
        { name: "timeoutMs", type: "number", required: false, description: "超时毫秒，默认 5000" },
      ],
    },
  ],
};

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const node = useNodeStore.getState();
  switch (action) {
    case "dial":
      return node.dial(String(args.target));
    case "connect":
      return node.connect(String(args.peerId));
    case "disconnect":
      return node.disconnect(String(args.peerId));
    case "ping": {
      const timeoutMs =
        typeof args.timeoutMs === "number" ? args.timeoutMs : 5000;
      return node.ping(String(args.peerId), timeoutMs);
    }
    default:
      throw new Error(`peers 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const snapshot = useNodeStore.getState();
  return {
    peers: selectPeerList(snapshot).map((peer) => ({
      peerId: peer.peerId,
      connected: peer.connected,
      source: peer.source,
      addrs: peer.addrs,
      lastSeenMs: peer.lastSeenMs,
    })),
  };
}

export const peersPage: PageEntry = { descriptor, execute, state };
