import { act, fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import { SettingsSaveBar } from "./save-bar";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function renderBar(overrides: {
  onSubmit?: () => Promise<void>;
  onSaveAndRestart?: () => Promise<void>;
  running?: boolean;
  onReportSaveError?: (error: unknown) => void;
  onReportRestartError?: (error: unknown) => void;
}) {
  return render(
    <SettingsSaveBar
      dirty
      loaded
      running={overrides.running ?? false}
      invalidCount={0}
      onSubmit={overrides.onSubmit ?? vi.fn(() => Promise.resolve())}
      onSaveAndRestart={overrides.onSaveAndRestart ?? vi.fn(() => Promise.resolve())}
      onReportSaveError={overrides.onReportSaveError ?? vi.fn()}
      onReportRestartError={overrides.onReportRestartError ?? vi.fn()}
    />,
  );
}

describe("SettingsSaveBar", () => {
  it("保存按钮 loading→success：进行中文案与状态图标切换", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const { getByRole, container } = renderBar({ onSubmit: () => d.promise });

    const save = getByRole("button", { name: "保存" });
    fireEvent.click(save);
    expect(save.getAttribute("aria-busy")).toBe("true");
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();
    // 长耗时等待期有进行中文案，不允许无反馈等待
    expect(getByRole("button", { name: "保存中…" })).toBeTruthy();

    await act(async () => {
      d.resolve(undefined);
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(container.querySelector(".lucide-check")).toBeTruthy();
    expect(getByRole("button", { name: "保存" })).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(container.querySelector(".lucide-check")).toBeFalsy();
  });

  it("失败进入 fail 态并上报，驻留后回 idle 可再次点击", async () => {
    vi.useFakeTimers();
    const d1 = deferred<void>();
    const onReportSaveError = vi.fn();
    const onSubmit = vi.fn(() => d1.promise);
    const { getByRole, container } = renderBar({ onSubmit, onReportSaveError });

    fireEvent.click(getByRole("button", { name: "保存" }));
    await act(async () => {
      d1.reject(new Error("disk full"));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(container.querySelector(".lucide-x")).toBeTruthy();
    expect(onReportSaveError).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(container.querySelector(".lucide-x")).toBeFalsy();

    // fail 态可恢复：按钮回到 idle 后可再次点击
    const d2 = deferred<void>();
    onSubmit.mockReturnValue(d2.promise);
    fireEvent.click(getByRole("button", { name: "保存" }));
    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(getByRole("button", { name: "保存中…" })).toBeTruthy();
  });

  it("保存并重启按钮 loading 期显示组合动作进行中文案", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const { getByRole, container } = renderBar({
      running: true,
      onSaveAndRestart: () => d.promise,
    });

    fireEvent.click(getByRole("button", { name: "保存并重启" }));
    expect(getByRole("button", { name: "保存并重启中…" })).toBeTruthy();
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();

    await act(async () => {
      d.resolve(undefined);
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(container.querySelector(".lucide-check")).toBeTruthy();
  });
});
