// 权限流增补测试（P1-ADD 11/12）：超时判定下沉 store（组件卸载也拒绝）、
// 通用拒绝仅在无 reject_* 选项时回 cancelled、多档选项完整渲染。
import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WsLike } from "./ws-factory";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { emptyInteraction } = await import("./interaction-model");
const { renderConnected, permissionId, sendPrompt, resetFixtures } = await import(
  "./acp-view-test-utils"
);
const { setWsFactory } = await import("./ws-factory");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
  RecorderSocket.sent.length = 0;
});

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
});

/** 打开即在线并记录上行帧的 socket：线级断言 sweeper 的 cancelled 应答 */
class RecorderSocket implements WsLike {
  static sent: string[] = [];
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  constructor() {
    window.setTimeout(() => this.onopen?.(), 0);
  }
  send(data: string): void {
    RecorderSocket.sent.push(data);
  }
  close(): void {}
}

describe("AcpView permission timeout in store", () => {
  it("pending 权限超时判定下沉 store：无组件挂载也自动拒绝并回 cancelled", async () => {
    vi.useFakeTimers();
    try {
      setWsFactory(() => new RecorderSocket());
      act(() => {
        useAcpStore.getState().connect();
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(20);
      });
      act(() => {
        useAcpStore.setState((s) => ({
          interactions: {
            ...s.interactions,
            "s-x": {
              ...emptyInteraction(),
              permissions: [
                {
                  requestId: 55,
                  sessionId: "s-x",
                  title: "Run tests",
                  toolKind: null,
                  options: [],
                  receivedAt: Date.now() - 61_000,
                  status: "pending" as const,
                },
              ],
            },
          },
        }));
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1_100);
      });
      const req = useAcpStore.getState().interactions["s-x"].permissions[0];
      expect(req.status).toBe("rejected");
      const frame = RecorderSocket.sent.find((d) => d.includes('"id":55'));
      expect(frame).toBeTruthy();
      expect(frame).toContain('"cancelled"');
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("AcpView permission options", () => {
  it("多档选项完整渲染，点 allow_always 回对应 selected outcome", async () => {
    mockAcpConsole.configure({
      promptScript: [
        {
          kind: "permission",
          toolKind: "execute",
          title: "Run tests",
          options: [
            { optionId: "allow-once", name: "Allow once", kind: "allow_once" },
            { optionId: "allow-always", name: "Allow always", kind: "allow_always" },
            { optionId: "reject-once", name: "Deny once", kind: "reject_once" },
          ],
        },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    expect(screen.getByTestId("acp-permission-option-" + id + "-allow-once")).toBeTruthy();
    expect(screen.getByTestId("acp-permission-option-" + id + "-allow-always")).toBeTruthy();
    expect(screen.getByTestId("acp-permission-option-" + id + "-reject-once")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-permission-option-" + id + "-allow-always"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已批准");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "selected", optionId: "allow-always" },
    });
  });

  it("无 reject 选项时渲染通用拒绝，点击回 cancelled", async () => {
    mockAcpConsole.configure({
      promptScript: [
        {
          kind: "permission",
          toolKind: "execute",
          title: "Run tests",
          options: [{ optionId: "allow-once", name: "Allow once", kind: "allow_once" }],
        },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    expect(screen.queryByTestId("acp-permission-option-" + id + "-reject-once")).toBeNull();
    fireEvent.click(screen.getByTestId("acp-permission-reject-" + id));
    await waitFor(() => {
      expect(screen.getByTestId("acp-permission-status-" + id).textContent).toContain("已拒绝");
    });
    expect(mockAcpConsole.responses.find((r) => r.id === id)?.result).toEqual({
      outcome: { outcome: "cancelled" },
    });
  });
});
