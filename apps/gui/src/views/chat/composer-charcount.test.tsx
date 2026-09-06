import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Composer } from "@/components/chat/composer";

import "@/i18n";

// F28（UX 审计 20260907）：输入框 2000 字上限的接近提示——≥1900 显字数
// 计数；超限就地红色提示且不清空内容、禁用发送；低于阈值不渲染计数。

function mountComposer(): HTMLTextAreaElement {
  render(
    <Composer peer="peer-count" replyTarget={null} onReplyCancel={() => {}} />,
  );
  return screen.getByTestId("chat-input") as HTMLTextAreaElement;
}

function typeInto(input: HTMLTextAreaElement, text: string): void {
  fireEvent.change(input, { target: { value: text } });
}

describe("F28 字数计数与超限提示", () => {
  it("低于 1900 不渲染计数", () => {
    const input = mountComposer();
    typeInto(input, "a".repeat(1899));
    expect(screen.queryByTestId("chat-char-count")).toBeNull();
    expect(screen.queryByTestId("chat-text-too-long")).toBeNull();
  });

  it("达到 1900 起显示计数，2000 内为中性色且可发送", () => {
    const input = mountComposer();
    typeInto(input, "a".repeat(1900));
    const counter = screen.getByTestId("chat-char-count");
    expect(counter.textContent).toBe("1900/2000");
    expect(screen.queryByTestId("chat-text-too-long")).toBeNull();
    typeInto(input, "a".repeat(2000));
    expect(counter.textContent).toBe("2000/2000");
    expect(screen.queryByTestId("chat-text-too-long")).toBeNull();
    expect(screen.getByTestId("chat-send")).not.toBeDisabled();
  });

  it("超限就地提示不清空内容：计数转红、发送禁用、回车不发送不清空", () => {
    const input = mountComposer();
    typeInto(input, "a".repeat(2001));
    const counter = screen.getByTestId("chat-char-count");
    expect(counter.textContent).toBe("2001/2000");
    expect(screen.getByTestId("chat-text-too-long").textContent).toContain("2000");
    expect(screen.getByTestId("chat-send")).toBeDisabled();
    fireEvent.keyDown(input, { key: "Enter" });
    expect((screen.getByTestId("chat-input") as HTMLTextAreaElement).value.length).toBe(2001);
  });
});
