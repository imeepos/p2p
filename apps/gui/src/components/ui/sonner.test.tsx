// 全局 toast 关闭行为（sonner v2 无 closeOnClick，AppToaster 事件委托实现）：
// 点击 toast 本体关单条；按钮/选中文本不触发；键盘 Enter 同样可关。
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { ThemeProvider } from "@/theme/theme-provider";

import { AppToaster } from "./sonner";

function renderToaster() {
  return render(
    <ThemeProvider>
      <AppToaster position="top-right" />
    </ThemeProvider>,
  );
}

/** 在显（未在退出动画中）的 toast 条数 */
function liveToasts(): NodeListOf<Element> {
  return document.querySelectorAll('[data-sonner-toast]:not([data-removed="true"])');
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  act(() => {
    toast.dismiss();
  });
});

describe("AppToaster 点击/键盘关闭（事件委托）", () => {
  it("点击 toast 本体关闭该条，另一条不受影响", async () => {
    renderToaster();
    act(() => {
      toastError("第一条错误", { description: "d1" });
      toastSuccess("第二条成功");
    });
    await waitFor(() => expect(liveToasts().length).toBe(2));
    // sonner 新条在前：按文案定位错误条再点
    const errorToast = [...liveToasts()].find((el) => el.textContent?.includes("第一条错误"))!;
    fireEvent.click(errorToast.querySelector("[data-title]") ?? errorToast);
    await waitFor(() => expect(liveToasts().length).toBe(1));
    expect(liveToasts()[0]!.textContent).toContain("第二条成功");
  });

  it("正选中文字（复制意图）时点击不关闭", async () => {
    renderToaster();
    act(() => {
      toastSuccess("保持显示");
    });
    await waitFor(() => expect(liveToasts().length).toBe(1));
    vi.spyOn(window, "getSelection").mockReturnValue({
      toString: () => "被选中的文本",
    } as Selection);
    fireEvent.click(liveToasts()[0]!.querySelector("[data-title]")!);
    await new Promise((r) => setTimeout(r, 100));
    expect(liveToasts().length).toBe(1);
  });

  it("键盘 Enter 在聚焦的 toast 上同样可关闭", async () => {
    renderToaster();
    act(() => {
      toastSuccess("键盘关闭");
    });
    await waitFor(() => expect(liveToasts().length).toBe(1));
    fireEvent.keyDown(liveToasts()[0]!, { key: "Enter" });
    await waitFor(() => expect(liveToasts().length).toBe(0));
  });

  it("点击 toast 内动作按钮（复制详情）不误关，由按钮自身语义收场", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    renderToaster();
    act(() => {
      toastError("复制场景", { description: "detail-x", context: "test.ctx" });
    });
    await waitFor(() => expect(liveToasts().length).toBe(1));
    const copyButton = [...document.querySelectorAll("[data-sonner-toast] button")].find(
      (b) => b.textContent === "复制详情",
    );
    expect(copyButton).toBeTruthy();
    fireEvent.click(copyButton!);
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
  });
});
