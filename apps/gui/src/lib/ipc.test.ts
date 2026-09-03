import { afterEach, describe, expect, it, vi } from "vitest";

// 诊断面固定走真实 Tauri IPC（2026-09-03 裁决）：即使 VITE_MOCK_IPC=1，
// diag 也不得回落 mock-diagnostics 读 localStorage——mock 仅测试内 vi.mock 使用。
const invokeMock = vi.hoisted(() =>
  vi.fn((cmd: string) => {
    if (cmd === "frontend_log_path") return Promise.resolve("/tmp/frontend.log");
    if (cmd === "frontend_log_tail") return Promise.resolve(["real-tail"]);
    return Promise.resolve(null);
  }),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

describe("ipc 诊断面路由", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    invokeMock.mockClear();
    localStorage.clear();
  });

  it("VITE_MOCK_IPC=1 时 diag 仍调用真实 frontend_log IPC", async () => {
    // 埋一个 mock 诊断会返回的内容：若 diag 误回落 localStorage，会读到它。
    localStorage.setItem("p2p-console.frontend-log", '{"kind":"mock-stale"}');
    vi.stubEnv("VITE_MOCK_IPC", "1");
    const { diag } = await import("./ipc");

    await expect(diag.logPath()).resolves.toBe("/tmp/frontend.log");
    await expect(diag.logTail(7)).resolves.toEqual(["real-tail"]);
    expect(invokeMock).toHaveBeenCalledWith("frontend_log_path");
    expect(invokeMock).toHaveBeenCalledWith("frontend_log_tail", { maxLines: 7 });
  });

  it("节点控制面在 VITE_MOCK_IPC=1 仍走 mock（与诊断面分离）", async () => {
    vi.stubEnv("VITE_MOCK_IPC", "1");
    const { ipc } = await import("./ipc");

    const status = await ipc.nodeStatus();
    expect(status).toMatchObject({ running: false });
    expect(invokeMock.mock.calls.map(([cmd]) => cmd)).not.toContain("node_status");
  });
});
