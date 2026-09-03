import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import "@/i18n";

const mocks = vi.hoisted(() => ({
  identityReset: vi.fn(),
  refresh: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { identityReset: mocks.identityReset },
}));
vi.mock("@/stores/node-store", () => ({
  useNodeStore: { getState: () => ({ refresh: mocks.refresh }) },
}));
vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: mocks.toastSuccess,
  toastError: mocks.toastError,
}));

import { ResetIdentityDialog } from "./reset-identity-dialog";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function typePrefix(prefix: string) {
  fireEvent.change(
    screen.getByPlaceholderText("输入当前 PeerId 前 4 位以确认"),
    { target: { value: prefix } },
  );
}

// fake timers 下 findBy/waitFor 的内部轮询定时器被冻结，Radix 弹框经
// fireEvent(act) 同步挂载，直接同步断言。
function openDialog() {
  fireEvent.click(screen.getByRole("button", { name: "重置身份" }));
  expect(screen.getByText("重置节点身份？")).toBeInTheDocument();
}

describe("ResetIdentityDialog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.identityReset.mockReset();
    mocks.refresh.mockReset();
    mocks.refresh.mockResolvedValue(undefined);
    mocks.toastSuccess.mockClear();
    mocks.toastError.mockClear();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("前缀不匹配时确认按钮禁用，匹配后方可点击", async () => {
    render(<ResetIdentityDialog peerId="abcd1234" />);
    await openDialog();
    const confirm = screen.getByRole("button", { name: "确认重置" });
    expect(confirm).toBeDisabled();
    typePrefix("abcd");
    expect(confirm).not.toBeDisabled();
  });

  it("成功路径：关弹框、清输入、toast 成功并刷新节点状态", async () => {
    mocks.identityReset.mockReturnValue(Promise.resolve());
    render(<ResetIdentityDialog peerId="abcd1234" />);
    await openDialog();
    typePrefix("abcd");
    fireEvent.click(screen.getByRole("button", { name: "确认重置" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.identityReset).toHaveBeenCalledWith(true);
    expect(mocks.refresh).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "身份已重置，节点已停止",
    );
    expect(
      screen.queryByText("重置节点身份？"),
    ).not.toBeInTheDocument();

    // 重新打开时输入框已清空，前缀需重新输入
    await openDialog();
    expect(screen.getByRole("button", { name: "确认重置" })).toBeDisabled();
  });

  it("loading 防重入：进行中重复点击不重复调用 IPC", async () => {
    const d = deferred<void>();
    mocks.identityReset.mockReturnValue(d.promise);
    render(<ResetIdentityDialog peerId="abcd1234" />);
    await openDialog();
    typePrefix("abcd");
    const confirm = screen.getByRole("button", { name: "确认重置" });
    fireEvent.click(confirm);
    expect(screen.getByRole("button", { name: "重置中…" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重置中…" }));
    expect(mocks.identityReset).toHaveBeenCalledTimes(1);
    await act(async () => {
      d.resolve(undefined);
      await vi.advanceTimersByTimeAsync(300);
    });
  });

  it("失败路径：toast 带原因、弹框留驻，回 idle 后可重试成功", async () => {
    const d1 = deferred<void>();
    mocks.identityReset.mockReturnValue(d1.promise);
    render(<ResetIdentityDialog peerId="abcd1234" />);
    await openDialog();
    typePrefix("abcd");
    fireEvent.click(screen.getByRole("button", { name: "确认重置" }));
    await act(async () => {
      d1.reject(new Error("ipc unavailable"));
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.toastError).toHaveBeenCalledWith(
      "重置身份失败",
      "ipc unavailable",
    );
    // 弹框留驻，用户可重试
    expect(screen.getByText("重置节点身份？")).toBeInTheDocument();

    // fail 态驻留结束回 idle：重试成功关弹框
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1200);
    });
    const d2 = deferred<void>();
    mocks.identityReset.mockReturnValue(d2.promise);
    fireEvent.click(screen.getByRole("button", { name: "确认重置" }));
    expect(mocks.identityReset).toHaveBeenCalledTimes(2);
    await act(async () => {
      d2.resolve(undefined);
      await vi.advanceTimersByTimeAsync(300);
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "身份已重置，节点已停止",
    );
    expect(
      screen.queryByText("重置节点身份？"),
    ).not.toBeInTheDocument();
  });
});
