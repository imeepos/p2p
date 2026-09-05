// F3 行为测试（P1）：新权限请求到达 → sonner toast 提醒（带会话标识），
// 权限面板自动滚动进入视口；后续权限继续提醒，帧去重不重复提醒。
import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { toastMock } = vi.hoisted(() => ({ toastMock: vi.fn() }));
vi.mock("sonner", () => ({
  toast: Object.assign(toastMock, { success: vi.fn(), error: vi.fn(), dismiss: vi.fn() }),
}));

const { mockAcpConsole } = await import("./mock-acp-ws");
const { renderConnected, permissionId, sendPrompt, resetFixtures } = await import(
  "./acp-view-test-utils"
);
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
  toastMock.mockClear();
  // resetFixtures 已置 vi.fn 占位，这里换成带调用记录的 spy 供断言
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
});

function scrollSpy() {
  return HTMLElement.prototype.scrollIntoView as unknown as ReturnType<typeof vi.fn>;
}

describe("AcpView permission arrival notice", () => {
  it("权限到达：toast 带会话标识与请求标题，面板滚入视口", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "permission", toolKind: "execute", title: "Run tests" },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    const id = await permissionId();
    await screen.findByTestId("acp-permission-row-" + id);
    await waitFor(() => {
      expect(toastMock).toHaveBeenCalledTimes(1);
    });
    const [message, options] = toastMock.mock.calls[0] as [string, { description?: string }];
    expect(message).toContain("s-001");
    expect(options.description).toContain("Run tests");
    expect(scrollSpy()).toHaveBeenCalled();
    expect(scrollSpy().mock.calls[0][0]).toMatchObject({ block: "nearest" });
  });

  it("第二条权限再提醒一次：seq 自增，逐条通知不吞", async () => {
    mockAcpConsole.configure({
      promptScript: [
        { kind: "permission", toolKind: "execute", title: "First" },
        { kind: "permission", toolKind: "execute", title: "Second" },
        { kind: "stop", reason: "end_turn" },
      ],
    });
    await renderConnected();
    await sendPrompt();
    await vi.waitFor(() => {
      expect(toastMock).toHaveBeenCalledTimes(2);
    });
    expect((toastMock.mock.calls[0] as unknown[])[0]).toContain("s-001");
    expect((toastMock.mock.calls[1] as unknown[])[0]).toContain("s-001");
    expect((toastMock.mock.calls[1] as [string, { description?: string }])[1].description).toBe(
      "Second",
    );
  });
});
