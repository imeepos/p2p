import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const emitMock = vi.fn<(event: string, payload: unknown) => Promise<void>>();
vi.mock("@tauri-apps/api/event", () => ({
  emit: (event: string, payload: unknown) => emitMock(event, payload),
}));

import { installPageBridge, PAGE_REPLY_EVENT } from "./page-bridge";

function request(payload: unknown): void {
  window.__P2P_PAGES__?.request(typeof payload === "string" ? payload : JSON.stringify(payload));
}

beforeEach(() => {
  emitMock.mockReset();
  emitMock.mockResolvedValue(undefined);
  delete window.__P2P_PAGES__;
  window.location.hash = "";
});

afterEach(() => {
  delete window.__P2P_PAGES__;
});

describe("installPageBridge", () => {
  it("重复安装幂等", () => {
    installPageBridge();
    const bridge = window.__P2P_PAGES__;
    installPageBridge();
    expect(window.__P2P_PAGES__).toBe(bridge);
  });
});

describe("describe 请求", () => {
  it("按当前 hash 路由返回 descriptor（含 state）", async () => {
    window.location.hash = "#/chat";
    installPageBridge();
    request({ requestId: "r1", kind: "describe" });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [event, reply] = emitMock.mock.calls[0]!;
    expect(event).toBe(PAGE_REPLY_EVENT);
    expect(reply).toMatchObject({ requestId: "r1", ok: true });
    expect((reply as { data: { name: string } }).data.name).toBe("chat");
  });

  it("未注册页回结构化 PAGE_NOT_REGISTERED", async () => {
    installPageBridge();
    request({ requestId: "r2", kind: "describe" });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect(reply).toMatchObject({
      requestId: "r2",
      ok: false,
      error: { code: "PAGE_NOT_REGISTERED" },
    });
  });

  it("显式 page 参数优先于 hash", async () => {
    window.location.hash = "#/chat";
    installPageBridge();
    request({ requestId: "r3", kind: "describe", page: "peers" });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect((reply as { data: { name: string } }).data.name).toBe("peers");
  });
});

describe("execute 请求", () => {
  it("缺 action 回 INVALID_REQUEST，不触达执行器", async () => {
    installPageBridge();
    request({ requestId: "r4", kind: "execute", page: "chat" });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect(reply).toMatchObject({
      requestId: "r4",
      ok: false,
      error: { code: "INVALID_REQUEST" },
    });
  });

  it("未知动作回 ACTION_NOT_FOUND", async () => {
    installPageBridge();
    request({ requestId: "r5", kind: "execute", page: "chat", action: "nope", args: {} });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect(reply).toMatchObject({
      requestId: "r5",
      ok: false,
      error: { code: "ACTION_NOT_FOUND" },
    });
  });

  it("危险动作缺 confirm 回 ACTION_CONFIRM_REQUIRED", async () => {
    installPageBridge();
    request({
      requestId: "r6",
      kind: "execute",
      page: "chat",
      action: "removeFriend",
      args: { peer: "p1" },
    });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect(reply).toMatchObject({
      requestId: "r6",
      ok: false,
      error: { code: "ACTION_CONFIRM_REQUIRED" },
    });
  });

  it("未知请求类型回 INVALID_REQUEST", async () => {
    installPageBridge();
    request({ requestId: "r7", kind: "shutdown" });
    await vi.waitFor(() => expect(emitMock).toHaveBeenCalled());
    const [, reply] = emitMock.mock.calls[0]!;
    expect(reply).toMatchObject({ requestId: "r7", ok: false, error: { code: "INVALID_REQUEST" } });
  });
});

describe("失败可观测", () => {
  it("非法 JSON 不 emit，只留本地告警", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    installPageBridge();
    request("{not-json");
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(emitMock).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("emit 被拒时捕获并告警，不产生未处理拒绝", async () => {
    emitMock.mockRejectedValue(new Error("bridge-closed"));
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    window.location.hash = "#/chat";
    installPageBridge();
    request({ requestId: "r8", kind: "describe" });
    await vi.waitFor(() => expect(errorSpy).toHaveBeenCalled());
    errorSpy.mockRestore();
  });
});
