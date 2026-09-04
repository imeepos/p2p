import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMocks = vi.hoisted(() => ({
  configGet: vi.fn(),
  configSave: vi.fn(),
}));
const nodeState = vi.hoisted(() => ({
  metrics: null as Record<string, number> | null,
}));

vi.mock("@/lib/ipc", () => ({ ipc: ipcMocks }));
vi.mock("@/stores/node-store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/stores/node-store")>();
  return {
    ...actual,
    useNodeStore: { getState: () => nodeState, subscribe: vi.fn() },
  };
});

import { relayPage } from "./relay-page";
import { executePageAction } from "../page-registry";

const BASE_CONFIG = {
  quicPort: 4001,
  tcpPort: 4002,
  enableMdns: true,
  dataDir: "/tmp/p2p",
  bootstrap: [],
  relayAddrs: ["10.0.0.9/u3400"],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

beforeEach(() => {
  vi.clearAllMocks();
  ipcMocks.configGet.mockResolvedValue({ ...BASE_CONFIG });
  ipcMocks.configSave.mockImplementation(async (cfg: unknown) => cfg);
  nodeState.metrics = null;
});

describe("relay 页 descriptor", () => {
  it("descriptor 快照与动作清单", () => {
    expect(relayPage.descriptor).toMatchSnapshot();
    expect(relayPage.descriptor.actions.map((a) => a.name)).toEqual([
      "saveRelayAddrs",
    ]);
  });

  it("state 与水位卡/逐跳统计卡同源（store metrics）", () => {
    nodeState.metrics = {
      relaySessionsActive: 3,
      relayReconnects: 1,
      dialPunchOk: 5,
      dialPunchFail: 2,
      dialRelayOk: 7,
      dialRelayFail: 1,
    };
    const snapshot = relayPage.state?.() as Record<string, unknown>;
    expect(snapshot).toEqual({
      relaySessionsActive: 3,
      relayReconnects: 1,
      dialPunch: { ok: 5, fail: 2 },
      dialRelay: { ok: 7, fail: 1 },
    });
    nodeState.metrics = null;
    expect(relayPage.state?.()).toMatchObject({ relaySessionsActive: null });
  });

  it("saveRelayAddrs 缺参结构化拒绝（ARG_MISSING）", async () => {
    await expect(executePageAction("relay", "saveRelayAddrs", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_MISSING" },
    });
    expect(ipcMocks.configSave).not.toHaveBeenCalled();
  });

  it("saveRelayAddrs 非法地址在校验层拒绝，零写入（与表单 zod 校验同源）", async () => {
    await expect(
      executePageAction("relay", "saveRelayAddrs", { relayAddrs: ["invalid-addr"] }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("addrFormat") },
    });
    expect(ipcMocks.configGet).not.toHaveBeenCalled();
    expect(ipcMocks.configSave).not.toHaveBeenCalled();
  });

  it("saveRelayAddrs 重复地址拒绝", async () => {
    await expect(
      executePageAction("relay", "saveRelayAddrs", {
        relayAddrs: ["10.0.0.1/u3400", "10.0.0.1/u3400"],
      }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("addrDuplicate") },
    });
  });

  it("saveRelayAddrs 成功走 configGet -> configSave（与保存按钮同源）", async () => {
    const result = await executePageAction("relay", "saveRelayAddrs", {
      relayAddrs: ["192.168.1.10/u3403"],
    });
    expect(result).toMatchObject({ ok: true });
    expect(ipcMocks.configSave).toHaveBeenCalledWith(
      expect.objectContaining({ relayAddrs: ["192.168.1.10/u3403"] }),
    );
  });
});
