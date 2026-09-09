// 消息条目一键复制（气泡外侧悬停钮）：复制值必须去掉首尾空白
// （空格/回车/换行/制表符等）；媒体与全空白文本不提供复制入口。
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  writeText: vi.fn<(text: string) => Promise<void>>(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: mocks.toastSuccessMock,
  toastError: mocks.toastErrorMock,
}));

import { MessageBubble } from "./message-bubble";
import { peerId, textMessage } from "@/test/chat-boundaries-fixtures";
import "@/i18n";

const PEER = peerId("copy-test");

beforeEach(() => {
  mocks.writeText.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: mocks.writeText },
  });
});

afterEach(() => cleanup());

describe("消息条目一键复制", () => {
  it("点击复制写入去掉首尾空白/回车换行/制表符后的文本", async () => {
    render(
      <MessageBubble
        message={textMessage("m1", PEER, "  \n\t 你好 世界 \r\n\t ")}
      />,
    );
    fireEvent.click(screen.getByTestId("message-copy-m1"));
    await waitFor(() =>
      expect(mocks.writeText).toHaveBeenCalledWith("你好 世界"),
    );
  });

  it("them 侧消息同样提供复制入口并写剪贴板", async () => {
    render(
      <MessageBubble
        message={textMessage("m2", PEER, "\n 内容 \n", { sender: "them" })}
      />,
    );
    fireEvent.click(screen.getByTestId("message-copy-m2"));
    await waitFor(() => expect(mocks.writeText).toHaveBeenCalledWith("内容"));
  });

  it("中间换行保留，仅去首尾", async () => {
    render(
      <MessageBubble message={textMessage("m3", PEER, "  a\nb  ")} />,
    );
    fireEvent.click(screen.getByTestId("message-copy-m3"));
    await waitFor(() => expect(mocks.writeText).toHaveBeenCalledWith("a\nb"));
  });

  it("媒体消息与全空白文本不渲染复制钮", () => {
    const { rerender } = render(
      <MessageBubble
        message={{
          id: "m4",
          peer: PEER,
          sender: "me",
          kind: "image",
          tsMs: 1,
          text: null,
          media: {
            name: "a.png",
            mime: "image/png",
            size: 3,
            path: "asset://localhost/a.png",
          },
          status: "delivered",
        }}
      />,
    );
    expect(screen.queryByTestId("message-copy-m4")).toBeNull();
    rerender(
      <MessageBubble message={textMessage("m5", PEER, " \n\t ")} />,
    );
    expect(screen.queryByTestId("message-copy-m5")).toBeNull();
  });
});
