import type { NodeEventJson } from "@/lib/ipc-types";
import { eventTimeMs } from "./event-clock";

// 导出 JSON：附带兜底后的 tsMs，浏览器侧触发文件下载。
export function exportEventsJson(events: NodeEventJson[]): void {
  const payload = events.map((event) => ({
    ...event,
    tsMs: eventTimeMs(event),
  }));
  const blob = new Blob([JSON.stringify(payload, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `p2p-events-${Date.now()}.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}
