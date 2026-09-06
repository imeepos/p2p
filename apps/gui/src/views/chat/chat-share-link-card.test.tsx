// 聊天消息分享链接卡片测试（渲染层识别，不改 ChatEnvelope/线协议）：
// 独占链接消息整体成卡、嵌在句中保留上下文文字、无链接不渲染卡片、
// 点击走与「用链接加入」相同的 /connect-share 导入路径。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { MessageBubble } = await import("@/components/chat/message-bubble");
const { useAcpStore } = await import("@/acp/acp-store");
const { resetFixtures } = await import("@/acp/acp-view-test-utils");
await import("@/i18n");
import type { ChatMessageJson } from "@/lib/ipc-types";

const PEER = "peerY";
const LINK = "dsh-acp-share://v1?peer=" + PEER + "&token=" + "b".repeat(32);

function messageOf(text: string): ChatMessageJson {
  return {
    id: "m-1",
    peer: "friend-1",
    sender: "them",
    kind: "text",
    tsMs: 1_700_000_000_000,
    text,
    status: "delivered",
  };
}

beforeEach(() => {
  resetFixtures();
  useAcpStore.getState().setDraft({
    statusUrl: "http://127.0.0.1:9900",
    token: "console-tok",
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("消息内分享链接卡片", () => {
  it("正文即链接：整条渲染为卡片，原始链接文本不再直出", () => {
    render(<MessageBubble message={messageOf(LINK)} />);
    expect(screen.getByTestId("chat-share-link-card")).toBeTruthy();
    expect(screen.queryByText(LINK)).toBeNull();
  });

  it("链接嵌在句中：上下文文字保留，链接位成卡", () => {
    render(<MessageBubble message={messageOf("看看这个 " + LINK + " 过期前用")} />);
    expect(screen.getByTestId("chat-share-link-card")).toBeTruthy();
    expect(screen.getByText("看看这个")).toBeTruthy();
    expect(screen.getByText("过期前用")).toBeTruthy();
  });

  it("无链接文本不渲染卡片（渲染层零扰动）", () => {
    render(<MessageBubble message={messageOf("普通消息")} />);
    expect(screen.queryByTestId("chat-share-link-card")).toBeNull();
    expect(screen.getByText("普通消息")).toBeTruthy();
  });

  it("点击卡片走同一导入路径：/connect-share 成功后目录可见 scope", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ ok: true, peer: PEER, scope: "sandbox" }),
      })),
    );
    render(<MessageBubble message={messageOf(LINK)} />);
    fireEvent.click(screen.getByTestId("chat-share-link-join"));
    await waitFor(() => {
      expect(screen.getByTestId("chat-share-link-card").textContent).toContain("已加入");
    });
    expect(useAcpStore.getState().directory.find((e) => e.peer === PEER)?.scope).toBe("sandbox");
  });
});
