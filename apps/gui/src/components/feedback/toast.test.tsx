import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Toaster, toast } from "sonner";
import { afterEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import {
  buildErrorDetailClipboard,
  toastError,
  type ToastErrorOptions,
} from "./toast";

// sonner 的 toast 挂载走异步 mount：同步 act 触发入队，findBy 等待渲染。
async function renderToastError(
  message: string,
  options?: string | ToastErrorOptions,
) {
  render(<Toaster position="bottom-right" />);
  act(() => {
    toastError(message, options);
  });
  return screen.findByRole("button", { name: "复制详情" });
}

function stubClipboard(writeImpl: () => Promise<void>) {
  const writeText = vi.fn(writeImpl);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  return writeText;
}

afterEach(() => {
  cleanup();
  act(() => {
    toast.dismiss();
  });
});

describe("toastError 复制详情", () => {
  it("错误 toast 提供复制详情入口，点击写入上下文+错误信息", async () => {
    const writeText = stubClipboard(() => Promise.resolve());
    const copyButton = await renderToastError("保存失败", {
      description: "disk full",
      context: "settings.config_save",
    });
    expect(copyButton).toBeTruthy();

    fireEvent.click(copyButton);
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    expect(writeText).toHaveBeenCalledWith(
      "context: settings.config_save\nerror: 保存失败\ndetail: disk full",
    );
    // 复制成功的轻反馈
    await screen.findByText("已复制到剪贴板");
  });

  it("二参 string 兼容形态：详情缺省取 description", async () => {
    const writeText = stubClipboard(() => Promise.resolve());
    const copyButton = await renderToastError("启动失败", "port in use");

    fireEvent.click(copyButton);
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    expect(writeText).toHaveBeenCalledWith(
      "error: 启动失败\ndetail: port in use",
    );
  });

  it("剪贴板写入失败留 console 信号并提示复制失败", async () => {
    const errorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    stubClipboard(() => Promise.reject(new Error("denied")));
    await renderToastError("停止失败", { description: "ipc timeout" });

    fireEvent.click(screen.getByRole("button", { name: "复制详情" }));
    await screen.findByText("复制失败");
    await waitFor(() => expect(errorSpy).toHaveBeenCalled());
    errorSpy.mockRestore();
  });
});

describe("buildErrorDetailClipboard", () => {
  it("组装 context/error/detail 三段，缺省段跳过", () => {
    expect(
      buildErrorDetailClipboard("boom", {
        context: "node.start",
        description: "e1",
      }),
    ).toBe("context: node.start\nerror: boom\ndetail: e1");
    expect(buildErrorDetailClipboard("boom", {})).toBe("error: boom");
    expect(
      buildErrorDetailClipboard("boom", { detail: "full stack" }),
    ).toBe("error: boom\ndetail: full stack");
  });
});
