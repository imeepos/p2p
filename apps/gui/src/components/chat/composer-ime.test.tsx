// UX5 回归：composer 回车发送的 IME 组合态守卫。组合中（组合事件进行中 /
// isComposing / keyCode 229）Enter 不发送；确认（compositionend）后回车正常发送。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc", () => ({ ipc: {} }));

import "@/i18n";

import { Composer } from "./composer";

function renderComposer() {
  const transport = {
    sendText: vi.fn<() => Promise<unknown>>().mockResolvedValue({}),
    sendMedia: vi.fn<() => Promise<unknown>>().mockResolvedValue({}),
  };
  render(
    <Composer
      peer="peer-1"
      replyTarget={null}
      onReplyCancel={() => {}}
      transport={transport}
    />,
  );
  const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
  return { transport, input };
}

describe("composer IME 组合态守卫（UX5）", () => {
  it("组合事件进行中按 Enter：不发送，草稿保留", () => {
    const { transport, input } = renderComposer();
    fireEvent.change(input, { target: { value: "你好" } });
    fireEvent.compositionStart(input);
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229, isComposing: true });
    expect(transport.sendText).not.toHaveBeenCalled();
    expect(input.value).toBe("你好");
  });

  it("keyCode 229 兜底：未挂组合事件也拦截（驱动不上报 isComposing 场景）", () => {
    const { transport, input } = renderComposer();
    fireEvent.change(input, { target: { value: "拼音" } });
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
    expect(transport.sendText).not.toHaveBeenCalled();
    expect(input.value).toBe("拼音");
  });

  it("isComposing=true 路径：无 compositionStart 也拦截", () => {
    const { transport, input } = renderComposer();
    fireEvent.change(input, { target: { value: "拼音" } });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(transport.sendText).not.toHaveBeenCalled();
  });

  it("compositionend 确认后按 Enter：正常发送并清空草稿", async () => {
    const { transport, input } = renderComposer();
    fireEvent.change(input, { target: { value: "你好" } });
    fireEvent.compositionStart(input);
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229, isComposing: true });
    expect(transport.sendText).not.toHaveBeenCalled();
    fireEvent.compositionEnd(input);
    fireEvent.keyDown(input, { key: "Enter", keyCode: 13 });
    await waitFor(() =>
      expect(transport.sendText).toHaveBeenCalledWith("peer-1", "你好", undefined),
    );
    await waitFor(() => expect(input.value).toBe(""));
  });
});
