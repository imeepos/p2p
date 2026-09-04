// dashboard 页 descriptor：动作与快速操作区（启动按钮 / 停止确认框）同源走
// node store；stop 断开全部连接，为危险动作，registry 强制 args.confirm===true。
import { useNodeStore } from "@/stores/node-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "dashboard",
  description: "仪表盘页：节点快速启停与运行状态概览",
  actions: [
    {
      name: "start",
      description: "启动节点（使用已加载的当前配置，与快速操作启动按钮同源）",
      args: [],
    },
    {
      name: "stop",
      description: "停止节点（断开全部连接，与快速操作停止确认框同源）",
      confirm: true,
      args: [
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

async function execute(
  action: string,
  _args: Record<string, unknown>,
): Promise<unknown> {
  const node = useNodeStore.getState();
  switch (action) {
    case "start": {
      if (!node.status) throw new Error("node status not loaded");
      return node.startNode(node.status.config);
    }
    case "stop":
      return node.stopNode();
    default:
      throw new Error(`dashboard 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const { status, metrics, subscriptionLive } = useNodeStore.getState();
  return {
    running: status?.running ?? null,
    peerId: status?.peerId ?? null,
    subscriptionLive,
    metrics,
  };
}

export const dashboardPage: PageEntry = { descriptor, execute, state };
