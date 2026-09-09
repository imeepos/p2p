// F1 行为测试（P0）：ACP 提示词输入框回车发送的组合态守卫。
// 组合选词期间（isComposing 或 keyCode 229 兜底）回车不得发送；
// 组合结束后的回车正常发送；Shift+Enter 换行行为不变。
import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

// S4 负载加固：默认 waitFor 预算 1s，高负载下 send→mock 回声链路实测 ~1.07s 撞悬崖
// 假红（同代码双跑一红一绿）。显式 10s 预算只放宽等待窗口，断言语义零改动。
const WAIT_TIMEOUT = 10_000;

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
    }, { timeout: WAIT_TIMEOUT });
    await screen.findByText("Hello from the mock agent.", {}, { timeout: WAIT_TIMEOUT });
    await waitFor(() => {
      expect(composer().value).toBe("");
    }, { timeout: WAIT_TIMEOUT });
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

  it("回合进行中发送禁用防重复提交，结算解除恢复；Stop 仅进行中可见", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: "draft" } });
    expect((screen.getByTestId("acp-composer-send") as HTMLButtonElement).disabled).toBe(false);
    act(() => {
      useAcpStore.setState({ promptPendingBySession: { "s-001": true } });
    });
    expect((screen.getByTestId("acp-composer-send") as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId("acp-composer-stop")).toBeTruthy();
    act(() => {
      useAcpStore.setState({ promptPendingBySession: {} });
    });
    expect((screen.getByTestId("acp-composer-send") as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByTestId("acp-composer-stop")).toBeNull();
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
