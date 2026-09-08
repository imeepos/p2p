import { afterEach, describe, expect, it, vi } from "vitest";

// 契约 §15：acp_console_status 命令映射与 acp-console 事件订阅（in-process
// pump 三态）；mock 面与真实桥同签名，相位经控制器可测可控。
const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

describe("ipc acp-console 状态面（契约 §15）", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it("真实桥：acp_console_status 逐字映射命令名，事件通道 acp-console", async () => {
    vi.resetModules();
    vi.stubEnv("VITE_MOCK_IPC", "0");
    invokeMock.mockResolvedValue({ phase: "connecting" });
    listenMock.mockResolvedValue(() => {});
    const { ipc } = await import("./ipc");

    await expect(ipc.acpConsoleStatus()).resolves.toMatchObject({ phase: "connecting" });
    expect(invokeMock).toHaveBeenCalledWith("acp_console_status");

    const seen: unknown[] = [];
    const unlisten = await ipc.onAcpConsoleEvent((s) => seen.push(s));
    expect(listenMock).toHaveBeenCalledWith("acp-console", expect.any(Function));
    expect(typeof unlisten).toBe("function");
  });

  it("mock 面：同签名取状态 + 订阅事件；emit 相位可控、退订后不再接收", async () => {
    vi.resetModules();
    vi.stubEnv("VITE_MOCK_IPC", "1");
    const { ipc } = await import("./ipc");
    const { mockAcpConsole } = await import("./mock-acp-console");
    mockAcpConsole.reset();

    await expect(ipc.acpConsoleStatus()).resolves.toMatchObject({ phase: "connected" });

    const seen: string[] = [];
    const unlisten = await ipc.onAcpConsoleEvent((s) => seen.push(s.phase));
    mockAcpConsole.emit({ phase: "disconnected", lastError: "boom" });
    mockAcpConsole.emit({ phase: "connected", wsUrl: "ws://127.0.0.1:1", token: "t" });
    expect(seen).toEqual(["disconnected", "connected"]);
    await expect(ipc.acpConsoleStatus()).resolves.toMatchObject({ phase: "connected" });

    unlisten();
    mockAcpConsole.emit({ phase: "disconnected" });
    expect(seen).toEqual(["disconnected", "connected"]);
    mockAcpConsole.reset();
  });

  it("mock 面非法相位钉值显式回退 disconnected（不静默）", async () => {
    vi.stubEnv("VITE_MOCK_IPC", "1");
    vi.stubEnv("VITE_MOCK_CONSOLE_PHASE", "bogus");
    vi.resetModules();
    const { ipc } = await import("./ipc");
    await expect(ipc.acpConsoleStatus()).resolves.toMatchObject({ phase: "disconnected" });
    vi.unstubAllEnvs();
  });
});
