import type { NodeEventJson } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";

// F18：相对时间统一走 lib/relative-time（未来时刻钳「刚刚」下限），
// 原调用方 import 路径不变。
export { formatRelative } from "@/lib/relative-time";

const receivedAt = new WeakMap<NodeEventJson, number>();
let armed = false;

function stamp(event: NodeEventJson, now: number): void {
  if (receivedAt.has(event)) return;
  // 契约 v1 §2：真实事件带发射时刻 tsMs；缺省（mock/旧载荷）用本地接收时间兜底。
  receivedAt.set(event, event.tsMs ?? now);
}

function arm(): void {
  if (armed) return;
  armed = true;
  useNodeStore.subscribe((state, prev) => {
    if (state.events === prev.events) return;
    const now = Date.now();
    for (const event of state.events) {
      // 新事件总在队首，遇到已记录的即可停。
      if (receivedAt.has(event)) break;
      stamp(event, now);
    }
  });
}

// 模块加载即挂订阅：store 收到事件的瞬间记录接收时间。
arm();

export function eventTimeMs(event: NodeEventJson): number {
  const known = receivedAt.get(event);
  if (known !== undefined) return known;
  stamp(event, Date.now());
  return receivedAt.get(event) as number;
}
