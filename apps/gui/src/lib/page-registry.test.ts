import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  chatFriendAdd: vi.fn(),
  chatFriendRemove: vi.fn(),
  identityReset: vi.fn(),
  configSave: vi.fn(),
}));

const chatState = vi.hoisted(() => ({
  selectedPeer: null as string | null,
  friends: [] as Array<{ peerId: string; nickname: string }>,
  sendText: vi.fn(),
  forgetFriend: vi.fn(),
  loadFriends: vi.fn(),
}));

const nodeState = vi.hoisted(() => ({
  status: null as { running: boolean; peerId: string | null } | null,
  peers: {} as Record<string, unknown>,
  dial: vi.fn(),
  connect: vi.fn(),
  disconnect: vi.fn(),
  ping: vi.fn(),
  stopNode: vi.fn(),
  startNode: vi.fn(),
  refresh: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({ ipc: mocks }));
vi.mock("@/stores/chat-store", () => ({
  useChatStore: { getState: () => chatState },
}));
vi.mock("@/stores/node-store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/stores/node-store")>();
  return { ...actual, useNodeStore: { getState: () => nodeState } };
});

import {
  PAGE_REGISTRY,
  describePage,
  executePageAction,
  type PageArgType,
} from "./page-registry";

const ARG_TYPES: PageArgType[] = ["string", "number", "boolean", "array", "object"];

beforeEach(() => {
  vi.clearAllMocks();
  chatState.selectedPeer = null;
  chatState.friends = [];
  nodeState.status = null;
  nodeState.peers = {};
});

describe("PAGE_REGISTRY 注册表校验", () => {
  it("登记 chat/peers/settings 三个核心页面", () => {
    expect(Object.keys(PAGE_REGISTRY).sort()).toEqual(["chat", "peers", "settings"]);
  });

  it.each(Object.entries(PAGE_REGISTRY))("页面 %s descriptor 结构合法", (page, entry) => {
    expect(entry.descriptor.name).toBe(page);
    expect(entry.descriptor.description.trim().length).toBeGreaterThan(0);
    expect(entry.descriptor.actions.length).toBeGreaterThan(0);
    expect(typeof entry.execute).toBe("function");
    const names = entry.descriptor.actions.map((action) => action.name);
    expect(new Set(names).size).toBe(names.length);
    for (const action of entry.descriptor.actions) {
      expect(action.description.trim().length).toBeGreaterThan(0);
      for (const arg of action.args) {
        expect(ARG_TYPES).toContain(arg.type);
        expect(arg.description.trim().length).toBeGreaterThan(0);
      }
    }
  });

  it("危险动作声明 confirm 且必须声明 boolean confirm 必填参数", () => {
    const confirmed: string[] = [];
    for (const [page, entry] of Object.entries(PAGE_REGISTRY)) {
      for (const action of entry.descriptor.actions) {
        if (!action.confirm) continue;
        confirmed.push(`${page}.${action.name}`);
        const arg = action.args.find((candidate) => candidate.name === "confirm");
        expect(arg, `${page}.${action.name} 缺 confirm 参数`).toMatchObject({
          type: "boolean",
          required: true,
        });
      }
    }
    expect(confirmed.sort()).toEqual([
      "chat.removeFriend",
      "settings.resetIdentity",
      "settings.saveAndRestart",
    ]);
  });

  it("chat 与 peers descriptor 快照", () => {
    expect(PAGE_REGISTRY.chat?.descriptor).toMatchSnapshot();
    expect(PAGE_REGISTRY.peers?.descriptor).toMatchSnapshot();
  });
});

describe("describePage", () => {
  it("返回 descriptor 并附当前状态快照", () => {
    chatState.selectedPeer = "peer-1";
    chatState.friends = [{ peerId: "peer-1", nickname: "Alice" }];
    const result = describePage("chat");
    expect("code" in result).toBe(false);
    if (!("code" in result)) {
      expect(result.descriptor.name).toBe("chat");
      expect(result.descriptor.state).toEqual({
        selectedPeer: "peer-1",
        friends: [{ peerId: "peer-1", nickname: "Alice" }],
      });
    }
  });

  it("peers 状态快照只含可见对端（复用表格可见性口径）", () => {
    nodeState.peers = {
      visible: {
        peerId: "visible",
        addrs: ["1.2.3.4/u4001"],
        source: "rendezvous",
        connected: false,
        lastSeenMs: 1,
      },
      hidden: {
        peerId: "hidden",
        addrs: ["127.0.0.1/u4001"],
        source: "rendezvous",
        connected: false,
        lastSeenMs: 2,
      },
    };
    const result = describePage("peers");
    if (!("code" in result)) {
      const rows = (result.descriptor.state as { peers: Array<{ peerId: string }> }).peers;
      expect(rows.map((row) => row.peerId)).toEqual(["visible"]);
    }
  });

  it("未注册页结构化报错", () => {
    const result = describePage("dashboard");
    expect(result).toMatchObject({ code: "PAGE_NOT_REGISTERED" });
  });
});

describe("executePageAction 拒绝路径", () => {
  it("未注册页 / 未知动作结构化拒绝", async () => {
    await expect(executePageAction("dashboard", "x", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "PAGE_NOT_REGISTERED" },
    });
    await expect(executePageAction("chat", "nope", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_NOT_FOUND" },
    });
  });

  it("危险动作缺 confirm / confirm 非 true 一律拒绝", async () => {
    for (const args of [{ peer: "p" }, { peer: "p", confirm: "yes" }, { peer: "p", confirm: 1 }]) {
      await expect(executePageAction("chat", "removeFriend", args)).resolves.toMatchObject({
        ok: false,
        error: { code: "ACTION_CONFIRM_REQUIRED" },
      });
    }
    expect(mocks.chatFriendRemove).not.toHaveBeenCalled();
  });

  it("参数缺失与类型不匹配结构化拒绝", async () => {
    await expect(executePageAction("peers", "dial", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_MISSING" },
    });
    await expect(executePageAction("peers", "dial", { target: 123 })).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_TYPE_MISMATCH" },
    });
  });
});

describe("executePageAction 真实执行器", () => {
  it("chat.sendText 走 store 发送", async () => {
    chatState.sendText.mockResolvedValue({ id: "m1" });
    const result = await executePageAction("chat", "sendText", { peer: "p1", text: "hi" });
    expect(result.ok).toBe(true);
    expect(chatState.sendText).toHaveBeenCalledWith("p1", "hi");
  });

  it("chat.addFriend 走 IPC + loadFriends（与添加好友表单同源）", async () => {
    mocks.chatFriendAdd.mockResolvedValue({ peerId: "p9", nickname: "N" });
    chatState.loadFriends.mockResolvedValue(undefined);
    const result = await executePageAction("chat", "addFriend", { peerId: " p9 ", addrs: ["1.2.3.4/u4001"] });
    expect(result.ok).toBe(true);
    expect(mocks.chatFriendAdd).toHaveBeenCalledWith("p9", "", ["1.2.3.4/u4001"]);
    expect(chatState.loadFriends).toHaveBeenCalled();
  });

  it("settings.saveConfig 走 configSave（与保存栏保存同源）", async () => {
    mocks.configSave.mockResolvedValue({ quicPort: 4001, tcpPort: 4002 });
    const result = await executePageAction("settings", "saveConfig", { config: { quicPort: 4001, tcpPort: 4002 } });
    expect(result.ok).toBe(true);
    expect(mocks.configSave).toHaveBeenCalledWith({ quicPort: 4001, tcpPort: 4002 });
  });

  it("chat.removeFriend 走 IPC + forgetFriend（与移除确认框同源）", async () => {
    mocks.chatFriendRemove.mockResolvedValue(true);
    const result = await executePageAction("chat", "removeFriend", {
      peer: "p1",
      confirm: true,
    });
    expect(result.ok).toBe(true);
    expect(mocks.chatFriendRemove).toHaveBeenCalledWith("p1");
    expect(chatState.forgetFriend).toHaveBeenCalledWith("p1");
  });

  it("peers.dial / ping 走 node store", async () => {
    nodeState.dial.mockResolvedValue({ ok: true });
    nodeState.ping.mockResolvedValue({ ok: true, rttMs: 3 });
    await expect(executePageAction("peers", "dial", { target: "1.2.3.4:4001" })).resolves.toMatchObject({
      ok: true,
    });
    expect(nodeState.dial).toHaveBeenCalledWith("1.2.3.4:4001");
    await executePageAction("peers", "ping", { peerId: "p1", timeoutMs: 1500 });
    expect(nodeState.ping).toHaveBeenCalledWith("p1", 1500);
    await executePageAction("peers", "ping", { peerId: "p2" });
    expect(nodeState.ping).toHaveBeenCalledWith("p2", 5000);
  });

  it("settings.resetIdentity 需 confirm 且执行后刷新状态", async () => {
    mocks.identityReset.mockResolvedValue({ peerId: "new-peer" });
    const result = await executePageAction("settings", "resetIdentity", { confirm: true });
    expect(result.ok).toBe(true);
    expect(mocks.identityReset).toHaveBeenCalledWith(true);
    expect(nodeState.refresh).toHaveBeenCalled();
  });

  it("settings.saveAndRestart 按 保存->stop->start 顺序执行", async () => {
    mocks.configSave.mockResolvedValue(undefined);
    nodeState.stopNode.mockResolvedValue({ running: false });
    nodeState.startNode.mockResolvedValue({ running: true, peerId: "p1" });
    const config = { quicPort: 0, tcpPort: 0, enableMdns: true };
    const result = await executePageAction("settings", "saveAndRestart", { config, confirm: true });
    expect(result.ok).toBe(true);
    expect(mocks.configSave).toHaveBeenCalledWith(config);
    const order = [
      mocks.configSave.mock.invocationCallOrder[0],
      nodeState.stopNode.mock.invocationCallOrder[0],
      nodeState.startNode.mock.invocationCallOrder[0],
    ];
    expect(order).toEqual([...order].sort((a, b) => a - b));
    expect(nodeState.startNode).toHaveBeenCalledWith(config);
  });

  it("执行器抛错映射 ACTION_FAILED", async () => {
    chatState.sendText.mockRejectedValue(new Error("boom"));
    await expect(executePageAction("chat", "sendText", { peer: "p1", text: "x" })).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: "boom" },
    });
  });
});
