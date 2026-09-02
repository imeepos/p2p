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
