import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import { ErrorBoundary } from "./error-boundary";
import { goToHomeRoute, reloadWindow } from "./window-actions";

vi.mock("./window-actions", () => ({
  goToHomeRoute: vi.fn(),
  reloadWindow: vi.fn(),
}));

function Bomb({ armed }: { armed: boolean }) {
  if (armed) throw new Error("boom");
  return <p>content-ok</p>;
}

function renderCrashed() {
  const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  const utils = render(
    <ErrorBoundary>
      <Bomb armed />
    </ErrorBoundary>,
  );
  return { errorSpy, ...utils };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ErrorBoundary 兜底出路", () => {
  it("崩溃时显示兜底页，含重试与两条出路按钮，文案走 i18n", () => {
    const { errorSpy } = renderCrashed();
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(screen.getByText("界面出错了")).toBeTruthy();
    expect(screen.getByRole("button", { name: "重试" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "返回首页" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "重载窗口" })).toBeTruthy();
    errorSpy.mockRestore();
  });

  it("重试保留：清空错误恢复子树", () => {
    const { errorSpy, rerender } = renderCrashed();
    rerender(
      <ErrorBoundary>
        <Bomb armed={false} />
      </ErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(screen.getByText("content-ok")).toBeTruthy();
    errorSpy.mockRestore();
  });

  it("返回首页可用：触发 Router 无关的首页导航并恢复子树", () => {
    const { errorSpy, rerender } = renderCrashed();
    rerender(
      <ErrorBoundary>
        <Bomb armed={false} />
      </ErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: "返回首页" }));
    expect(goToHomeRoute).toHaveBeenCalledTimes(1);
    expect(screen.getByText("content-ok")).toBeTruthy();
    errorSpy.mockRestore();
  });

  it("重载窗口按钮触发窗口重载", () => {
    const { errorSpy } = renderCrashed();
    fireEvent.click(screen.getByRole("button", { name: "重载窗口" }));
    expect(reloadWindow).toHaveBeenCalledTimes(1);
    errorSpy.mockRestore();
  });
});
