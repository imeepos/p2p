// events 页 descriptor：清空与导出与事件页控制条同源（store 公共 setState /
// exportEventsJson）。clear 清空缓冲不可恢复，为危险动作，registry 强制
// args.confirm===true；过滤条件属视图本地态，不进注册表。
import { useNodeStore } from "@/stores/node-store";
import { exportEventsJson } from "@/views/monitor/events-export";
import type { PageDescriptor, PageEntry } from "../page-registry";

const LATEST_ROWS = 10;

const descriptor: PageDescriptor = {
  name: "events",
  description: "事件页：节点事件缓冲的清空与导出",
  actions: [
    {
      name: "clear",
      description: "清空事件缓冲（与清空按钮确认框同源）",
      confirm: true,
      args: [
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
    {
      name: "export",
      description: "导出事件缓冲为 JSON 下载（与导出按钮同源 exportEventsJson）",
      args: [],
    },
  ],
};

async function execute(
  action: string,
  _args: Record<string, unknown>,
): Promise<unknown> {
  switch (action) {
    case "clear":
      useNodeStore.setState({ events: [] });
      return { cleared: true };
    case "export": {
      const events = useNodeStore.getState().events;
      exportEventsJson(events);
      return { exported: events.length };
    }
    default:
      throw new Error(`events 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const snapshot = useNodeStore.getState();
  return {
    subscriptionLive: snapshot.subscriptionLive,
    total: snapshot.events.length,
    latest: snapshot.events.slice(0, LATEST_ROWS),
  };
}

export const eventsPage: PageEntry = { descriptor, execute, state };
