import { describe, expect, it } from "vitest";

import { useNodeStore } from "@/stores/node-store";
import { eventTimeMs } from "./event-clock";
import type { NodeEventJson } from "@/lib/ipc-types";

describe("eventTimeMs", () => {
  it("优先采用载荷自带 tsMs（真实后端盖戳）", () => {
    const event: NodeEventJson = { type: "node_stopped", tsMs: 1234567890 };
    expect(eventTimeMs(event)).toBe(1234567890);
  });

  it("缺 tsMs 时以本地接收时间兜底（mock/旧载荷）", () => {
    const before = Date.now();
    const event: NodeEventJson = { type: "node_error", reason: "x" };
    const at = eventTimeMs(event);
    expect(at).toBeGreaterThanOrEqual(before);
    expect(at).toBeLessThanOrEqual(Date.now());
  });

  it("store 订阅路径：事件入队瞬间即盖戳，不依赖视图渲染", () => {
    const stamped: NodeEventJson = {
      type: "peer_connected",
      peer: "p1",
      tsMs: 42,
    };
    useNodeStore.setState({ events: [stamped] });
    expect(eventTimeMs(stamped)).toBe(42);

    const raw: NodeEventJson = { type: "peer_connected", peer: "p2" };
    const before = Date.now();
    useNodeStore.setState({ events: [raw] });
    expect(eventTimeMs(raw)).toBeGreaterThanOrEqual(before);
    useNodeStore.setState({ events: [] });
  });
});
