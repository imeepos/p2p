import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMocks = vi.hoisted(() => ({
  configGet: vi.fn(),
  configSave: vi.fn(),
}));
const nodeState = vi.hoisted(() => ({
  peers: {} as Record<string, unknown>,
}));

vi.mock("@/lib/ipc", () => ({ ipc: ipcMocks }));
vi.mock("@/stores/node-store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/stores/node-store")>();
  return {
    ...actual,
    useNodeStore: { getState: () => nodeState, subscribe: vi.fn() },
  };
});

import { discoveryPage } from "./discovery-page";
import { executePageAction } from "../page-registry";

const BASE_CONFIG = {
  quicPort: 4001,
  tcpPort: 4002,
  enableMdns: false,
  dataDir: "/tmp/p2p",
  bootstrap: ["10.0.0.1/u3400"],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

beforeEach(() => {
  vi.clearAllMocks();
  ipcMocks.configGet.mockResolvedValue({
    ...BASE_CONFIG,
    bootstrap: [...BASE_CONFIG.bootstrap],
  });
  ipcMocks.configSave.mockImplementation(async (cfg: unknown) => cfg);
  nodeState.peers = {};
});

describe("discovery 页 descriptor", () => {
  it("descriptor 快照与动作清单", () => {
    expect(discoveryPage.descriptor).toMatchSnapshot();
    expect(discoveryPage.descriptor.actions.map((a) => a.name)).toEqual([
      "setMdns",
      "addBootstrap",
      "removeBootstrap",
    ]);
  });

  it("state 与发现结果表同源（可见对端行）", () => {
    nodeState.peers = {
      visible: {
        peerId: "peer-a",
        addrs: ["1.2.3.4/u4001"],
        source: "rendezvous",
        connected: false,
        lastSeenMs: 1,
      },
    };
    const snapshot = discoveryPage.state?.() as {
      discovered: Array<{ peerId: string }>;
    };
    expect(snapshot.discovered.map((row) => row.peerId)).toEqual(["peer-a"]);
  });

  it("removeBootstrap 缺 confirm 结构化拒绝且零写入", async () => {
    await expect(
      executePageAction("discovery", "removeBootstrap", { addr: "10.0.0.1/u3400" }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_CONFIRM_REQUIRED" },
    });
    expect(ipcMocks.configSave).not.toHaveBeenCalled();
  });

  it("addBootstrap 非法地址在校验层拒绝，零写入（与对话框内联校验同源）", async () => {
    await expect(
      executePageAction("discovery", "addBootstrap", { addr: "not-an-addr" }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("addrFormat") },
    });
    expect(ipcMocks.configSave).not.toHaveBeenCalled();
  });

  it("addBootstrap 重复地址拒绝", async () => {
    await expect(
      executePageAction("discovery", "addBootstrap", { addr: "10.0.0.1/u3400" }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("addrDuplicate") },
    });
    expect(ipcMocks.configSave).not.toHaveBeenCalled();
  });

  it("addBootstrap 成功走 configGet -> configSave（trim 后追加）", async () => {
    const result = await executePageAction("discovery", "addBootstrap", {
      addr: " 192.168.1.10/u3400 ",
    });
    expect(result).toMatchObject({ ok: true });
    expect(ipcMocks.configSave).toHaveBeenCalledWith(
      expect.objectContaining({
        bootstrap: ["10.0.0.1/u3400", "192.168.1.10/u3400"],
      }),
    );
  });

  it("setMdns 与开关同源（configSave enableMdns）", async () => {
    const result = await executePageAction("discovery", "setMdns", { enable: true });
    expect(result).toMatchObject({ ok: true });
    expect(ipcMocks.configSave).toHaveBeenCalledWith(
      expect.objectContaining({ enableMdns: true }),
    );
  });

  it("removeBootstrap 带 confirm 删除既有地址；缺失地址结构化失败", async () => {
    const ok = await executePageAction("discovery", "removeBootstrap", {
      addr: "10.0.0.1/u3400",
      confirm: true,
    });
    expect(ok).toMatchObject({ ok: true });
    expect(ipcMocks.configSave).toHaveBeenCalledWith(
      expect.objectContaining({ bootstrap: [] }),
    );
    await expect(
      executePageAction("discovery", "removeBootstrap", {
        addr: "10.9.9.9/u1",
        confirm: true,
      }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("not found") },
    });
  });
});
