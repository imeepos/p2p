import { describe, expect, it } from "vitest";

import { reduceEvent, type EventStateSlice } from "./event-reducer";
import type { NodeStatus } from "@/lib/ipc-types";

const STATUS: NodeStatus = {
  running: false,
  peerId: null,
  listenAddrs: [],
  uptimeSecs: 0,
  startedAtMs: null,
  config: {
    quicPort: 0,
    tcpPort: 0,
    enableMdns: true,
    dataDir: "/tmp",
    bootstrap: [],
    relayAddrs: [],
    advertisedAddrs: [],
    observationPort: null,
    observationAddrs: [],
  },
};

function emptySlice(): EventStateSlice {
  return { events: [], peers: {}, status: STATUS, eventSeq: 0 };
}

describe("快速启停事件序列", () => {
  it("started/stopped 高频交替后终态与最后一条一致", () => {
    let s = emptySlice();
    for (let round = 0; round < 50; round += 1) {
      s = reduceEvent(s, { type: "node_started", listenAddrs: ["0.0.0.0/1"] });
      s = reduceEvent(s, { type: "node_stopped" });
    }
    expect(s.status?.running).toBe(false);
    expect(s.status?.listenAddrs).toEqual([]);

    s = reduceEvent(s, { type: "node_started", listenAddrs: ["0.0.0.0/2"] });
    expect(s.status).toMatchObject({ running: true, listenAddrs: ["0.0.0.0/2"] });
  });

  it("lag 型 node_error 不产生 peer 档且计入事件缓冲", () => {
    let s = emptySlice();
    s = reduceEvent(s, {
      type: "node_error",
      reason: "事件通道积压，已丢弃 7 条事件",
    });
    expect(Object.keys(s.peers)).toEqual([]);
    expect(s.events[0]?.type).toBe("node_error");
  });

  it("dial_failed 的 peer 为 null 时不建空档", () => {
    const s = reduceEvent(emptySlice(), {
      type: "dial_failed",
      peer: null,
      reason: "解析失败",
    });
    expect(Object.keys(s.peers)).toEqual([]);
  });
});
