import { fireEvent, render, act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AsyncButton } from "./async-button";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("AsyncButton", () => {
  it("idle 到 loading 到 success 再回 idle，含 300ms 防闪烁与 1.2s 驻留", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const { getByRole, container } = render(
      <AsyncButton action={() => d.promise}>Go</AsyncButton>,
    );
    const btn = getByRole("button");
    expect(btn.getAttribute("aria-busy")).toBe("false");

    fireEvent.click(btn);
    expect(btn.getAttribute("aria-busy")).toBe("true");
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();

    await act(async () => {
      d.resolve(undefined);
    });
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(299);
    });
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(container.querySelector(".lucide-check")).toBeTruthy();
    expect(btn.getAttribute("aria-busy")).toBe("false");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1199);
    });
    expect(container.querySelector(".lucide-check")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(container.querySelector(".lucide-check")).toBeFalsy();
    expect(container.querySelector(".lucide-loader-circle")).toBeFalsy();
  });

  it("action 拒绝进入 fail 态并回调 onError，之后回 idle", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const onError = vi.fn();
    const { getByRole, container } = render(
      <AsyncButton action={() => d.promise} onError={onError}>
        Go
      </AsyncButton>,
    );
    fireEvent.click(getByRole("button"));
    await act(async () => {
      d.reject(new Error("boom"));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(onError).toHaveBeenCalledTimes(1);
    expect(container.querySelector(".lucide-x")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(container.querySelector(".lucide-x")).toBeFalsy();
  });

  it("fail 态驻留结束回 idle 后可再次点击并成功", async () => {
    vi.useFakeTimers();
    const d1 = deferred<void>();
    const action = vi
      .fn()
      .mockImplementationOnce(() => d1.promise)
      .mockImplementationOnce(() => Promise.resolve("ok"));
    const { getByRole, container } = render(
      <AsyncButton action={action}>Go</AsyncButton>,
    );
    const btn = getByRole("button");
    fireEvent.click(btn);
    await act(async () => {
      d1.reject(new Error("boom"));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(container.querySelector(".lucide-x")).toBeTruthy();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(container.querySelector(".lucide-x")).toBeFalsy();

    // 恢复 idle：可再次点击并走到 success
    fireEvent.click(btn);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(action).toHaveBeenCalledTimes(2);
    expect(container.querySelector(".lucide-check")).toBeTruthy();
  });

  it("loading 期间 loadingLabel 替代 children", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const { getByRole } = render(
      <AsyncButton action={() => d.promise} loadingLabel="重启中…">
        保存并重启
      </AsyncButton>,
    );
    fireEvent.click(getByRole("button", { name: "保存并重启" }));
    expect(getByRole("button", { name: "重启中…" })).toBeTruthy();
    expect(() => getByRole("button", { name: "保存并重启" })).toThrow();
  });

  it("iconOnly busy 期间隐藏 children，只渲染状态图标", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const { getByRole, container } = render(
      <AsyncButton iconOnly action={() => d.promise}>
        <span data-testid="row-icon">X</span>
      </AsyncButton>,
    );
    const btn = getByRole("button");
    expect(container.querySelector('[data-testid="row-icon"]')).toBeTruthy();
    fireEvent.click(btn);
    expect(container.querySelector('[data-testid="row-icon"]')).toBeFalsy();
    expect(container.querySelector(".lucide-loader-circle")).toBeTruthy();
    await act(async () => {
      d.resolve(undefined);
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(container.querySelector('[data-testid="row-icon"]')).toBeFalsy();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    expect(container.querySelector('[data-testid="row-icon"]')).toBeTruthy();
  });

  it("busy 期间忽略重复点击", async () => {
    vi.useFakeTimers();
    const d = deferred<void>();
    const action = vi.fn(() => d.promise);
    const { getByRole } = render(<AsyncButton action={action}>Go</AsyncButton>);
    const btn = getByRole("button");
    fireEvent.click(btn);
    fireEvent.click(btn);
    expect(action).toHaveBeenCalledTimes(1);
    expect(btn.getAttribute("aria-busy")).toBe("true");
  });
});
