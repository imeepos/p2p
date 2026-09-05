import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const emitMock = vi.fn<(event: string, payload: unknown) => Promise<void>>();
vi.mock("@tauri-apps/api/event", () => ({
  emit: (event: string, payload: unknown) => emitMock(event, payload),
}));

import { installControlBridge, normalizeRoute } from "./control-bridge";

describe("normalizeRoute", () => {
  it("空 hash 归一化为 dashboard", () => {
    expect(normalizeRoute("")).toBe("dashboard");
    expect(normalizeRoute("#")).toBe("dashboard");
    expect(normalizeRoute("#/")).toBe("dashboard");
  });

  it("带斜杠与查询串的路由归一化", () => {
    expect(normalizeRoute("#/chat")).toBe("chat");
    expect(normalizeRoute("#/peers?x=1")).toBe("peers");
    expect(normalizeRoute("#settings")).toBe("settings");
  });

  it("/network/* 子路由按迁移别名归一化为原页面注册键", () => {
    expect(normalizeRoute("#/network/overview")).toBe("dashboard");
    expect(normalizeRoute("#/network/peers?x=1")).toBe("peers");
    expect(normalizeRoute("#/network/discovery")).toBe("discovery");
    expect(normalizeRoute("#/network/relay")).toBe("relay");
    expect(normalizeRoute("#/network/events")).toBe("events");
    expect(normalizeRoute("#/network/diagnostics")).toBe("diagnostics");
  });
});

describe("installControlBridge", () => {
  beforeEach(() => {
    emitMock.mockClear();
    emitMock.mockResolvedValue(undefined);
    delete window.__P2P_CONTROL_BRIDGE__;
  });

  afterEach(() => {
    delete window.__P2P_CONTROL_BRIDGE__;
  });

  it("安装即上报当前路由，hashchange 时再次上报", () => {
    window.location.hash = "#/chat";
    installControlBridge();
    expect(emitMock).toHaveBeenCalledWith("control-route", { route: "chat" });
    emitMock.mockClear();
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(emitMock).toHaveBeenCalledTimes(1);
    expect(emitMock).toHaveBeenCalledWith("control-route", { route: "chat" });
  });

  it("重复安装不重复上报", () => {
    window.location.hash = "#/chat";
    installControlBridge();
    installControlBridge();
    expect(emitMock).toHaveBeenCalledTimes(1);
  });

  it("上报被拒时只告警不抛出", async () => {
    emitMock.mockRejectedValue(new Error("bridge-closed"));
    installControlBridge();
    await Promise.resolve();
    expect(emitMock).toHaveBeenCalledTimes(1);
  });
});
