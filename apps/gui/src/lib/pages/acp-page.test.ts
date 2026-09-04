import { beforeEach, describe, expect, it, vi } from "vitest";

const acpState = vi.hoisted(() => ({
  phase: "idle" as string,
  sessions: [] as Array<{ sessionId: string; title?: string }>,
  activeSessionId: null as string | null,
  lastError: null as string | null,
  connect: vi.fn(async () => {}),
  disconnect: vi.fn(),
  newSession: vi.fn(async () => {}),
  refreshSessions: vi.fn(async () => {}),
  sendPrompt: vi.fn(async (_text: string) => {}),
  closeSession: vi.fn(async (_sessionId: string) => {}),
}));

vi.mock("@/acp/acp-store", () => ({
  useAcpStore: { getState: () => acpState },
}));

import { acpPage } from "./acp-page";
import { describePage, executePageAction } from "../page-registry";

const ACTION_NAMES = [
  "connect",
  "disconnect",
  "newSession",
  "refreshSessions",
  "sendPrompt",
  "closeSession",
];

beforeEach(() => {
  vi.clearAllMocks();
  acpState.phase = "idle";
  acpState.sessions = [];
  acpState.activeSessionId = null;
  acpState.lastError = null;
  acpState.connect.mockImplementation(async () => {});
  acpState.sendPrompt.mockImplementation(async () => {});
});

describe("acp 页 descriptor", () => {
  it("descriptor 快照与动作清单（R2 全量，无 confirm 危险标记）", () => {
    expect(acpPage.descriptor).toMatchSnapshot();
    expect(acpPage.descriptor.actions.map((a) => a.name)).toEqual(ACTION_NAMES);
    for (const action of acpPage.descriptor.actions) {
      expect(action.confirm).toBeUndefined();
    }
  });

  it("经注册表可达：describe 返回 R2 动作与 R3 state 三键", () => {
    acpState.phase = "online";
    acpState.sessions = [{ sessionId: "s1", title: "演示" }];
    acpState.activeSessionId = "s1";
    expect(describePage("acp")).toMatchObject({
      descriptor: {
        name: "acp",
        actions: expect.arrayContaining(
          ACTION_NAMES.map((name) => expect.objectContaining({ name })),
        ),
        state: {
          phase: "online",
          sessions: [{ sessionId: "s1", title: "演示" }],
          activeSessionId: "s1",
        },
      },
    });
  });

  it("state 只读派生：快照不触发任何 store action", () => {
    const snapshot = acpPage.state?.() as Record<string, unknown>;
    expect(snapshot).toEqual({ phase: "idle", sessions: [], activeSessionId: null });
    expect(acpState.connect).not.toHaveBeenCalled();
    expect(acpState.sendPrompt).not.toHaveBeenCalled();
    expect(acpState.closeSession).not.toHaveBeenCalled();
  });

  it("sendPrompt 缺 text 拒绝（ARG_MISSING）且不触达 store", async () => {
    await expect(executePageAction("acp", "sendPrompt", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_MISSING", message: expect.stringContaining("text") },
    });
    expect(acpState.sendPrompt).not.toHaveBeenCalled();
  });

  it("sendPrompt text 类型错误拒绝（ARG_TYPE_MISMATCH）", async () => {
    await expect(
      executePageAction("acp", "sendPrompt", { text: 42 }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_TYPE_MISMATCH" },
    });
    expect(acpState.sendPrompt).not.toHaveBeenCalled();
  });

  it("closeSession 缺 sessionId 拒绝（ARG_MISSING）", async () => {
    await expect(executePageAction("acp", "closeSession", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ARG_MISSING" },
    });
    expect(acpState.closeSession).not.toHaveBeenCalled();
  });

  it("refreshSessions 只读动作执行：透传清单且不触碰连接动作", async () => {
    acpState.sessions = [{ sessionId: "s9" }];
    const result = await executePageAction("acp", "refreshSessions", {});
    expect(result).toMatchObject({
      ok: true,
      data: { sessions: [{ sessionId: "s9" }], phase: "idle" },
    });
    expect(acpState.refreshSessions).toHaveBeenCalledTimes(1);
    expect(acpState.connect).not.toHaveBeenCalled();
    expect(acpState.disconnect).not.toHaveBeenCalled();
  });

  it("sendPrompt 成功：与 store.sendPrompt 同源传参并回传快照", async () => {
    acpState.activeSessionId = "s1";
    const result = await executePageAction("acp", "sendPrompt", { text: "你好" });
    expect(result).toMatchObject({ ok: true, data: { activeSessionId: "s1" } });
    expect(acpState.sendPrompt).toHaveBeenCalledWith("你好");
  });

  it("closeSession 成功：透传 sessionId", async () => {
    await expect(
      executePageAction("acp", "closeSession", { sessionId: "s1" }),
    ).resolves.toMatchObject({ ok: true });
    expect(acpState.closeSession).toHaveBeenCalledWith("s1");
  });

  it("disconnect 执行：与 store.disconnect 同源", async () => {
    await expect(executePageAction("acp", "disconnect", {})).resolves.toMatchObject({
      ok: true,
    });
    expect(acpState.disconnect).toHaveBeenCalledTimes(1);
  });

  it("store 新增 lastError 转 ACTION_FAILED 结构化返回", async () => {
    acpState.sendPrompt.mockImplementation(async () => {
      acpState.lastError = "promptFailed";
    });
    await expect(
      executePageAction("acp", "sendPrompt", { text: "你好" }),
    ).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED", message: expect.stringContaining("promptFailed") },
    });
  });

  it("connect 在端点不全时经 lastError 结构化失败", async () => {
    acpState.connect.mockImplementation(async () => {
      acpState.lastError = "endpointIncomplete";
    });
    await expect(executePageAction("acp", "connect", {})).resolves.toMatchObject({
      ok: false,
      error: {
        code: "ACTION_FAILED",
        message: expect.stringContaining("endpointIncomplete"),
      },
    });
  });

  it("存量 lastError 不误伤：动作成功且无新增 lastError 时放行", async () => {
    acpState.lastError = "promptFailed";
    await expect(
      executePageAction("acp", "sendPrompt", { text: "你好" }),
    ).resolves.toMatchObject({ ok: true });
  });
});
