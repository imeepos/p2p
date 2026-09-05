// F2 行为测试（P1）：草稿按会话隔离。切换会话各显各的未发送内容，
// 切回原会话草稿仍在；发送成功只清当前会话草稿，杜绝把话发给另一个 agent。
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

async function resume(sessionId: string) {
  fireEvent.click(
    screen.getByTestId("acp-session-row-" + sessionId).querySelector("button")!,
  );
  await waitFor(() => {
    expect(useAcpStore.getState().activeSessionId).toBe(sessionId);
  });
}

describe("AcpView composer draft isolation", () => {
  it("两会话草稿互不污染，切回原会话草稿仍在", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: "给一号 agent 的话" } });
    fireEvent.click(screen.getByTestId("acp-session-new"));
    await screen.findByTestId("acp-session-row-s-002");
    // 切到 s-002：显示 s-002 自己的（空）草稿，不含 s-001 的内容
    expect(composer().value).toBe("");
    fireEvent.change(composer(), { target: { value: "给二号 agent 的话" } });
    await resume("s-001");
    expect(composer().value).toBe("给一号 agent 的话");
    await resume("s-002");
    expect(composer().value).toBe("给二号 agent 的话");
  });

  it("发送成功只清当前会话草稿，发送内容归属当前会话", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: "draft-a" } });
    fireEvent.click(screen.getByTestId("acp-session-new"));
    await screen.findByTestId("acp-session-row-s-002");
    fireEvent.change(composer(), { target: { value: "draft-b" } });
    fireEvent.click(screen.getByTestId("acp-composer-send"));
    await screen.findByText("Hello from the mock agent.");
    await waitFor(() => {
      expect(useAcpStore.getState().promptDrafts["s-002"]).toBeUndefined();
    });
    // s-001 的草稿原样保留
    expect(useAcpStore.getState().promptDrafts["s-001"]).toBe("draft-a");
    // s-002 的 transcript 收到的是它自己的草稿
    const turns = useAcpStore.getState().transcripts["s-002"].turns;
    expect(
      turns.some((t) => t.kind === "user" && (t as { text?: string }).text === "draft-b"),
    ).toBe(true);
  });

  it("关闭会话草稿随删，不残留到其他会话", async () => {
    await renderConnected();
    await newSession();
    fireEvent.change(composer(), { target: { value: " doomed" } });
    fireEvent.click(screen.getByTestId("acp-session-new"));
    await screen.findByTestId("acp-session-row-s-002");
    useAcpStore.setState({ activeSessionId: "s-001" });
    useAcpStore.getState().closeSession("s-001");
    await waitFor(() => {
      expect(useAcpStore.getState().promptDrafts["s-001"]).toBeUndefined();
    });
    expect(useAcpStore.getState().promptDrafts["s-002"]).toBeUndefined();
  });
});
