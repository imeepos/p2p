import { describe, expect, it } from "vitest";

import {
  MAX_EVENTS,
  MAX_HOPS_PER_PEER,
  reduceEvent,
  type EventStateSlice,
} from "./event-reducer";
import type { NodeEventJson, NodeStatus } from "@/lib/ipc-types";

const STATUS: NodeStatus = {
  running: false,
  peerId: "peerX",
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
  return { events: [], peers: {}, status: STATUS };
}

describe("reduceEvent", () => {
  it("peer_discovered 建档并记录来源，peer_connected/disconnected 翻转连接态", () => {
    let s = emptySlice();
    s = reduceEvent(s, {
      type: "peer_discovered",
      peer: "p1",
      addrs: ["1.2.3.4/3400"],
      source: "rendezvous",
    });
    expect(s.peers.p1).toMatchObject({
      connected: false,
      addrs: ["1.2.3.4/3400"],
      source: "rendezvous",
    });

    s = reduceEvent(s, { type: "peer_connected", peer: "p1" });
    expect(s.peers.p1.connected).toBe(true);

    s = reduceEvent(s, { type: "peer_disconnected", peer: "p1" });
    expect(s.peers.p1.connected).toBe(false);
  });

  it("lastSeenMs 只由正向证据刷新：manual 发现、dial_hop、disconnected 均不刷新", () => {
    let s = emptySlice();
    // manual 来源是本端自身登记，不证明对端存活：建档但保持 lastSeenMs=0
    s = reduceEvent(s, {
      type: "peer_discovered",
      peer: "manual-peer",
      addrs: ["1.2.3.4/3400"],
      source: "manual",
    });
    expect(s.peers["manual-peer"].lastSeenMs).toBe(0);

    // 发现源消息是正向证据
    s = reduceEvent(s, {
      type: "peer_discovered",
      peer: "fresh-peer",
      addrs: ["1.2.3.5/3400"],
      source: "mdns",
    });
    expect(s.peers["fresh-peer"].lastSeenMs).toBeGreaterThan(0);
    const freshAt = s.peers["fresh-peer"].lastSeenMs;

    // dial_hop（成功或失败）与 disconnected 都不是在线证据
    s = reduceEvent(s, {
      type: "dial_hop",
      peer: "fresh-peer",
      hop: "relay",
      ok: false,
      detail: "offline",
    });
    s = reduceEvent(s, { type: "peer_disconnected", peer: "fresh-peer" });
    expect(s.peers["fresh-peer"].hops.length).toBe(1);
    expect(s.peers["fresh-peer"].connected).toBe(false);
    expect(s.peers["fresh-peer"].lastSeenMs).toBe(freshAt);
  });

  it("dial_hop 前插逐跳历史且受 MAX_HOPS_PER_PEER 环形上限", () => {
    let s = emptySlice();
    const hopEvent: NodeEventJson = {
      type: "dial_hop",
      peer: "p1",
      hop: "relay",
      ok: true,
      detail: "d",
    };
    for (let i = 0; i < MAX_HOPS_PER_PEER + 5; i += 1) {
      s = reduceEvent(s, hopEvent);
    }
    expect(s.peers.p1.hops.length).toBe(MAX_HOPS_PER_PEER);
    expect(s.peers.p1.hops[0].detail).toBe("d");
  });

  it("node_started/stopped 翻转本地状态快照", () => {
    let s = emptySlice();
    s = reduceEvent(s, { type: "node_started", listenAddrs: ["0.0.0.0/34000"] });
    expect(s.status?.running).toBe(true);
    expect(s.status?.listenAddrs).toEqual(["0.0.0.0/34000"]);

    s = reduceEvent(s, { type: "node_stopped" });
    expect(s.status?.running).toBe(false);
    expect(s.status?.listenAddrs).toEqual([]);
  });

  it("事件环形缓冲上限 1000，新事件在前", () => {
    let s: EventStateSlice = { ...emptySlice(), status: null };
    for (let i = 0; i < MAX_EVENTS + 50; i += 1) {
      s = reduceEvent(s, { type: "node_stopped" });
    }
    expect(s.events.length).toBe(MAX_EVENTS);
    expect(MAX_EVENTS).toBe(1000);
  });

  it("未知类型外的状态事件不影响 peers 表", () => {
    const s = reduceEvent(emptySlice(), { type: "node_error", reason: "x" });
    expect(Object.keys(s.peers)).toEqual([]);
    expect(s.events.length).toBe(1);
  });
});
