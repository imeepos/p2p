// resolveSessionCwd 单测：Tauri 态取主目录并缓存；非 Tauri 回退占位并留痕；失败路径告警。
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// hoisted 共享实例：vi.resetModules 后动态 import 重求值 mock 工厂，fn 引用不漂移
const mocks = vi.hoisted(() => ({ homeDir: vi.fn() }));
vi.mock("@tauri-apps/api/path", () => ({ homeDir: mocks.homeDir }));

describe("resolveSessionCwd", () => {
  beforeEach(() => {
    vi.resetModules();
    mocks.homeDir.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    delete window.__TAURI_INTERNALS__;
  });

  it("非 Tauri（mock/测试默认）：回退占位路径并 info 留痕", async () => {
    const { resolveSessionCwd } = await import("./session-cwd");
    const infoSpy = vi.spyOn(console, "info").mockImplementation(() => {});
    await expect(resolveSessionCwd()).resolves.toBe("/tmp/p2p-gui-mock-cwd");
    expect(infoSpy.mock.calls.some((c) => String(c[0]).includes("非 Tauri"))).toBe(true);
  });

  it("Tauri 态：取 homeDir 并缓存（二次调用不再触 API）", async () => {
    window.__TAURI_INTERNALS__ = {};
    mocks.homeDir.mockResolvedValue("/Users/tester");
    const { resolveSessionCwd } = await import("./session-cwd");
    await expect(resolveSessionCwd()).resolves.toBe("/Users/tester");
    await expect(resolveSessionCwd()).resolves.toBe("/Users/tester");
    expect(mocks.homeDir).toHaveBeenCalledTimes(1);
  });

  it("Tauri 态 homeDir 抛错：warn 留痕并回退占位路径，不阻塞会话", async () => {
    window.__TAURI_INTERNALS__ = {};
    mocks.homeDir.mockRejectedValue(new Error("path plugin down"));
    const { resolveSessionCwd } = await import("./session-cwd");
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    await expect(resolveSessionCwd()).resolves.toBe("/tmp/p2p-gui-mock-cwd");
    expect(warnSpy.mock.calls.some((c) => String(c[0]).includes("主目录解析失败"))).toBe(true);
  });
});
