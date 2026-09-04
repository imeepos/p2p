import { vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import type { GuiConfig, NodeEventJson } from "./ipc-types";

// 群 mock 测试共享工具（行数纪律拆分）：seed/事件收集/节点启停。
// mock-ipc 的群与好友 state 是模块级单例且 vitest 按文件隔离模块——
// 「空列表」断言必须位于文件首个 groupList 调用，用例用独立 seed 隔离。

export const TEST_CFG: GuiConfig = {
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

export const TEST_ADDR = "192.168.1.5/u3400";
const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

export function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

export function groupOf(seed: string): string {
  return `00000000-0000-4000-8000-${seed.padStart(12, "0").slice(-12)}`;
}

export async function startNode(): Promise<string> {
  const start = mockBackend.nodeStart(TEST_CFG);
  await vi.advanceTimersByTimeAsync(1000);
  const status = await start;
  return status.peerId!;
}

export async function stopIfRunning(): Promise<void> {
  const status = await mockBackend.nodeStatus();
  if (status.running) {
    const stop = mockBackend.nodeStop();
    await vi.advanceTimersByTimeAsync(500);
    await stop;
  }
}

export async function addFriend(seed: string): Promise<string> {
  const peer = peerId(seed);
  await mockBackend.chatFriendAdd(peer, `好友${seed}`, [TEST_ADDR]);
  return peer;
}

// 已知节点行内拨号：直连一步成功，把成员置为 connected（fan-out ack 前置）。
export async function connect(peer: string): Promise<void> {
  const dial = mockBackend.peerConnect(peer);
  await vi.advanceTimersByTimeAsync(1000);
  await dial;
}

// 事件收集器：注册 node-event 监听并按 type 过滤断言。
export function collectGroupEvents() {
  let events: NodeEventJson[] = [];
  let unlisten: (() => void) | null = null;
  return {
    async listen() {
      unlisten = await mockBackend.onNodeEvent((event) => events.push(event));
    },
    release() {
      unlisten?.();
      unlisten = null;
    },
    reset() {
      events = [];
    },
    eventsOf(type: NodeEventJson["type"]): NodeEventJson[] {
      return events.filter((e) => e.type === type);
    },
  };
}
