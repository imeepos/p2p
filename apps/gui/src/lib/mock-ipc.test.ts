import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import type { GuiConfig } from "./ipc-types";

const CFG: GuiConfig = {
  quicPort: 34000,
  tcpPort: 34001,
  enableMdns: true,
  dataDir: "/tmp/mock",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

const TICK_MS = 2500;

async function stopIfRunning() {
  const status = await mockBackend.nodeStatus();
  if (status.running) {
    const stop = mockBackend.nodeStop();
    await vi.advanceTimersByTimeAsync(500);
    await stop;
  }
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(async () => {
  await stopIfRunning();
  vi.useRealTimers();
});

describe("mock-ipc", () => {
  it("nodeStart 模拟启动延迟并先发 node_started，随后周期事件流动", async () => {
    const events: string[] = [];
    const unlisten = await mockBackend.onNodeEvent((e) => events.push(e.type));
    const started = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(799);
    expect(events).toEqual([]);

    await vi.advanceTimersByTimeAsync(1);
    const status = await started;
    expect(status.running).toBe(true);
    expect(events[0]).toBe("node_started");
    expect(status.listenAddrs).toContain("0.0.0.0/34000");

    await vi.advanceTimersByTimeAsync(TICK_MS * 12);
    const flowed = new Set(events);
    expect(
      flowed.has("peer_discovered") ||
        flowed.has("peer_connected") ||
        flowed.has("dial_hop"),
    ).toBe(true);

    const stopped = mockBackend.nodeStop();
    await vi.advanceTimersByTimeAsync(500);
    const stoppedStatus = await stopped;
    expect(stoppedStatus.running).toBe(false);
    unlisten();
  });

  it("nodeStart 已运行时拒绝（幂等性反向）", async () => {
    const first = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    await first;
    await expect(mockBackend.nodeStart(CFG)).rejects.toThrow(/已在运行/);
    await stopIfRunning();
  });

  it("peerDial 语法预检同桥接规则：坏 target 抛 Err，合法 target 返回逐跳报告", async () => {
    await expect(mockBackend.peerDial("no-at-sign")).rejects.toThrow(
      /target 语法非法/,
    );
    // 无 u/t 前缀：桥接层 proto::parse_target 同样拒绝（本次集成对齐的缺口）
    await expect(
      mockBackend.peerDial("abc123@192.168.1.5/3400"),
    ).rejects.toThrow(/target 语法非法/);

    const peer = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";
    const start = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    await start;
    const dial = mockBackend.peerDial(`${peer}@192.168.1.5/u3400`);
    await vi.advanceTimersByTimeAsync(2000);
    const report = await dial;
    expect(report.peer).toBe(peer);
    expect(report.hops.length).toBeGreaterThan(0);
    expect(report.totalMs).toBeGreaterThan(0);
    expect(typeof report.ok).toBe("boolean");
    await stopIfRunning();
  });

  it("peerConnect/peerDisconnect：未知节点失败报告，已知节点连接与幂等挂断", async () => {
    const peer = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";
    await expect(mockBackend.peerConnect(peer)).rejects.toThrow(/节点未运行/);
    await expect(mockBackend.peerDisconnect(peer)).rejects.toThrow(/节点未运行/);

    const start = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    await start;

    const miss = mockBackend.peerConnect(
      "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    );
    await vi.advanceTimersByTimeAsync(1000);
    const missReport = await miss;
    expect(missReport.ok).toBe(false);
    expect(missReport.hops[0]?.detail).toMatch(/no known address/);

    const dial = mockBackend.peerDial(`${peer}@192.168.1.5/u3400`);
    await vi.advanceTimersByTimeAsync(2000);
    await dial;

    const events: string[] = [];
    const unlisten = await mockBackend.onNodeEvent((e) => events.push(e.type));

    const connect = mockBackend.peerConnect(peer);
    await vi.advanceTimersByTimeAsync(1000);
    const report = await connect;
    expect(report.ok).toBe(true);
    expect(report.hops[0]?.ok).toBe(true);

    const hang = mockBackend.peerDisconnect(peer);
    await vi.advanceTimersByTimeAsync(300);
    expect(await hang).toBe(true);
    const again = mockBackend.peerDisconnect(peer);
    await vi.advanceTimersByTimeAsync(300);
    expect(await again).toBe(false);
    expect(events).toContain("peer_disconnected");
    unlisten();
    await stopIfRunning();
  });

  it("peerPing 未运行抛错；未知节点返回失败 outcome；reset 需显式 confirm", async () => {
    await expect(mockBackend.peerPing("zzz", 1000)).rejects.toThrow(
      /节点未运行/,
    );

    const start = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    await start;

    const pingAssert = expect(
      mockBackend.peerPing("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", 1000),
    ).resolves.toMatchObject({ ok: false });
    await vi.advanceTimersByTimeAsync(1000);
    await pingAssert;

    await expect(mockBackend.identityReset(false)).rejects.toThrow(
      /confirm=true/,
    );
    await stopIfRunning();
  });

  it("configGet/configSave 往返一致", async () => {
    expect(await mockBackend.configGet()).toEqual(CFG);
    const save = mockBackend.configSave({ ...CFG, quicPort: 12345 });
    await vi.advanceTimersByTimeAsync(300);
    const saved = await save;
    expect(saved.quicPort).toBe(12345);
  });
});
