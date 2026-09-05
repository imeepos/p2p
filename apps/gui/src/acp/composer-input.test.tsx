// F1 行为测试（P0）：ACP 提示词输入框回车发送的组合态守卫。
// 组合选词期间（isComposing 或 keyCode 229 兜底）回车不得发送；
// 组合结束后的回车正常发送；Shift+Enter 换行行为不变。
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { useAcpStore } = await import("./acp-store");
const { renderConnected, newSession, resetFixtures } = await import("./acp-view-test-utils");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

function composer() {
  return screen.getByTestId("acp-composer-input") as HTMLTextAreaElement;
}

function pendingOf(sessionId: string): boolean {
  return useAcpStore.getState().promptPendingBySession[sessionId] ?? false;
}

describe("AcpView composer IME guard", () => {
  it("组合态回车（isComposing）不发送；组合结束后的回车正常发送", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: "你好世界" } });
    fireEvent.keyDown(composer(), { key: "Enter", isComposing: true });
    expect(pendingOf("s-001")).toBe(false);
    expect(composer().value).toBe("你好世界");
    // compositionend 之后的回车：正常发送并清空草稿
    fireEvent.keyDown(composer(), { key: "Enter" });
    await waitFor(() => {
      expect(pendingOf("s-001")).toBe(true);
    });
    await screen.findByText("Hello from the mock agent.");
    await waitFor(() => {
      expect(composer().value).toBe("");
    });
  });

  it("keyCode 229 兜底：未置 isComposing 的组合态回车同样不发送", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: "选词中" } });
    fireEvent.keyDown(composer(), { key: "Enter", keyCode: 229, which: 229 });
    expect(pendingOf("s-001")).toBe(false);
    expect(composer().value).toBe("选词中");
    expect(useAcpStore.getState().transcripts["s-001"]?.turns.length ?? 0).toBe(0);
  });

  it("Shift+Enter 换行不发送（守卫不改变既有行为）", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: "draft" } });
    fireEvent.keyDown(composer(), { key: "Enter", shiftKey: true });
    expect(composer().value).toBe("draft");
    expect(pendingOf("s-001")).toBe(false);
  });
});
