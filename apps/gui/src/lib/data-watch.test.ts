import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn<(cmd: string, args?: unknown) => Promise<unknown>>();
type ListenHandler = (event: { payload: unknown }) => void;
const handlers = new Map<string, ListenHandler[]>();
const listenMock = vi.fn<(event: string, handler: ListenHandler) => Promise<() => void>>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: ListenHandler) => listenMock(event, handler),
}));

import {
  DATA_DOMAINS,
  ECHO_SUPPRESS_MS,
  markLocalWrite,
  registerReloader,
  resetForTest,
  setNowFnForTest,
  startDataWatch,
  useDataWatchStore,
} from "./data-watch";

function emit(event: string, payload: unknown): void {
  for (const handler of handlers.get(event) ?? []) handler({ payload });
}

describe("data-watch（W1 前端单监听器）", () => {
  let now = 1000;
  beforeEach(() => {
    resetForTest();
    handlers.clear();
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    listenMock.mockReset();
    listenMock.mockImplementation((event, handler) => {
      const list = handlers.get(event) ?? [];
      list.push(handler);
      handlers.set(event, list);
      return Promise.resolve(() => undefined);
    });
    now = 1000;
    setNowFnForTest(() => now);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("startDataWatch 安装 data-changed 与 data-watch-status 两个监听", async () => {
    await startDataWatch();
    await startDataWatch();
    expect(listenMock).toHaveBeenCalledTimes(2);
    expect(handlers.has("data-changed")).toBe(true);
    expect(handlers.has("data-watch-status")).toBe(true);
  });

  it("data-changed 按域定向分发：只触发注册域的 reloader", async () => {
    await startDataWatch();
    const configReload = vi.fn();
    const chatReload = vi.fn();
    const offConfig = registerReloader("config", configReload);
    registerReloader("chat", chatReload);
    emit("data-changed", { domains: ["config"] });
    expect(configReload).toHaveBeenCalledTimes(1);
    expect(chatReload).not.toHaveBeenCalled();
    offConfig();
    emit("data-changed", { domains: ["config"] });
    expect(configReload).toHaveBeenCalledTimes(1);
    expect(useDataWatchStore.getState().appliedCount).toBe(2);
  });

  it("未知域与非法载荷被忽略", async () => {
    await startDataWatch();
    const reload = vi.fn();
    registerReloader("profile", reload);
    emit("data-changed", { domains: ["unknown", "p2p-data"] });
    emit("data-changed", {});
    expect(reload).not.toHaveBeenCalled();
    expect(useDataWatchStore.getState().appliedCount).toBe(0);
    expect(DATA_DOMAINS).toHaveLength(3);
  });

  it("自身写回声抑制：markLocalWrite 后窗口内同域事件跳过重载", async () => {
    await startDataWatch();
    const reload = vi.fn();
    registerReloader("profile", reload);
    markLocalWrite("profile");
    emit("data-changed", { domains: ["profile"] });
    expect(reload).not.toHaveBeenCalled();
    expect(useDataWatchStore.getState().suppressedCount).toBe(1);
    // 窗口外（时钟前移）恢复生效
    now += ECHO_SUPPRESS_MS + 1;
    emit("data-changed", { domains: ["profile"] });
    expect(reload).toHaveBeenCalledTimes(1);
    expect(useDataWatchStore.getState().appliedCount).toBe(1);
  });

  it("抑制按域独立：自身写 config 不影响 chat 域刷新", async () => {
    await startDataWatch();
    const chatReload = vi.fn();
    registerReloader("chat", chatReload);
    markLocalWrite("config");
    emit("data-changed", { domains: ["chat"] });
    expect(chatReload).toHaveBeenCalledTimes(1);
  });

  it("生效分发落 frontend.log 感知证据（E2E 断言面）", async () => {
    await startDataWatch();
    registerReloader("config", () => undefined);
    emit("data-changed", { domains: ["config"] });
    await Promise.resolve();
    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledWith("frontend_log_append", {
      lines: [expect.stringContaining('"kind":"data-changed"')],
    });
    const arg = invokeMock.mock.calls[0][1] as { lines: string[] };
    expect(arg.lines[0]).toContain("\"domains\":[\"config\"]");
  });

  it("data-watch-status active:false 置降级态（R3 可判）", async () => {
    await startDataWatch();
    emit("data-watch-status", { active: false, reason: "watch failed" });
    expect(useDataWatchStore.getState().degraded).toBe(true);
    expect(useDataWatchStore.getState().reason).toBe("watch failed");
    emit("data-watch-status", { active: true });
    expect(useDataWatchStore.getState().degraded).toBe(false);
  });

  it("监听安装失败置降级且不抛出", async () => {
    listenMock.mockRejectedValueOnce(new Error("bridge-closed"));
    await expect(startDataWatch()).resolves.toBeUndefined();
    expect(useDataWatchStore.getState().degraded).toBe(true);
    expect(useDataWatchStore.getState().reason).toBe("bridge-closed");
  });
});