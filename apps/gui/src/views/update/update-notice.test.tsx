import { act, fireEvent, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateCheckResult } from "@/lib/ipc-types";

const { toastMessageMock, openPageMock } = vi.hoisted(() => ({
  toastMessageMock: vi.fn(),
  openPageMock: vi.fn<() => Promise<void>>(),
}));

vi.mock("sonner", () => ({ toast: { message: toastMessageMock } }));
vi.mock("@/lib/ipc", () => ({
  ipc: { updateCheck: vi.fn(), updateOpenReleasePage: openPageMock },
}));

import "@/i18n";
import { useUpdateStore } from "@/stores/update-store";
import { UpdateNotice } from "./update-notice";

const FIXTURE: UpdateCheckResult = {
  currentVersion: "0.1.0",
  latestVersion: "0.2.0",
  hasUpdate: true,
  releaseUrl: "https://github.com/imeepos/p2p/releases/tag/client-v0.2.0",
  releaseName: "p2p-console client-v0.2.0",
  releaseNotesMd: "## notes",
  publishedAtMs: Date.parse("2026-09-01T10:00:00Z"),
  checkedAtMs: Date.now(),
};

function setAvailable(over: Partial<UpdateCheckResult> = {}): void {
  useUpdateStore.setState({
    status: "available",
    result: { ...FIXTURE, ...over },
    error: null,
    lastSource: "auto",
    skippedVersion: null,
    reminderShownFor: null,
  });
}

beforeEach(() => {
  localStorage.clear();
  toastMessageMock.mockClear();
  openPageMock.mockClear();
  setAvailable();
});

describe("UpdateNotice 提醒 toast", () => {
  it("有更新且未跳过：弹一次提醒并带查看详情 action", () => {
    render(<UpdateNotice />);
    expect(toastMessageMock).toHaveBeenCalledTimes(1);
    const [title, options] = toastMessageMock.mock.calls[0];
    expect(title).toBe("发现新版本 0.2.0");
    expect(options.action.label).toBe("查看详情");
  });

  it("StrictMode 双执行 effect 与结果对象刷新均不重复提醒", () => {
    // StrictMode 挂载期 effect 双执行：第二次靠 getState 去重挡下
    const { rerender } = render(
      <StrictMode>
        <UpdateNotice />
      </StrictMode>,
    );
    // 模拟 4h 轮询：新 checkedAtMs 生成新 result 对象，版本未变，
    // reminderShownFor 不随轮询重置
    act(() => {
      useUpdateStore.setState({
        status: "available",
        result: { ...FIXTURE, checkedAtMs: FIXTURE.checkedAtMs + 1 },
      });
    });
    rerender(
      <StrictMode>
        <UpdateNotice />
      </StrictMode>,
    );
    expect(toastMessageMock).toHaveBeenCalledTimes(1);
  });

  it("跳过该版本后不再提醒；换新版本恢复提醒", () => {
    act(() => {
      useUpdateStore.setState({ skippedVersion: "0.2.0" });
    });
    render(<UpdateNotice />);
    expect(toastMessageMock).not.toHaveBeenCalled();

    act(() => {
      useUpdateStore.setState({
        skippedVersion: null,
        result: { ...FIXTURE, latestVersion: "0.3.0" },
      });
    });
    expect(toastMessageMock).toHaveBeenCalledTimes(1);
    expect(toastMessageMock.mock.calls[0][0]).toBe("发现新版本 0.3.0");
  });

  it("无更新不提醒", () => {
    useUpdateStore.setState({
      status: "upToDate",
      result: { ...FIXTURE, latestVersion: "0.1.0", hasUpdate: false },
    });
    render(<UpdateNotice />);
    expect(toastMessageMock).not.toHaveBeenCalled();
  });
});

describe("UpdateNotice 详情对话框", () => {
  it("查看详情打开对话框：版本号/发布名/说明/时间齐全，前往下载走白名单命令", async () => {
    render(<UpdateNotice />);
    const action = toastMessageMock.mock.calls[0][1].action;
    act(() => {
      action.onClick();
    });
    expect(await screen.findByText("新版本详情")).toBeInTheDocument();
    expect(screen.getByText("0.2.0")).toBeInTheDocument();
    // 发布名在对话框描述与详情行各出现一次
    expect(screen.getAllByText("p2p-console client-v0.2.0").length).toBeGreaterThan(0);
    expect(screen.getByText("发布说明")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "前往下载" }));
    expect(openPageMock).toHaveBeenCalledTimes(1);
    expect(openPageMock).toHaveBeenCalledWith(FIXTURE.releaseUrl);
  });
});
