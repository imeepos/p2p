import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateCheckResult } from "@/lib/ipc-types";

const { updateCheckMock } = vi.hoisted(() => ({
  updateCheckMock: vi.fn<() => Promise<UpdateCheckResult>>(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { updateCheck: updateCheckMock, updateOpenReleasePage: vi.fn() },
}));

import "@/i18n";
import { useUpdateStore } from "@/stores/update-store";
import { AboutUpdateCard } from "./about-update-card";

function availableResult(): UpdateCheckResult {
  return {
    currentVersion: "0.1.0",
    latestVersion: "0.2.0",
    hasUpdate: true,
    releaseUrl: "https://github.com/imeepos/p2p/releases/tag/client-v0.2.0",
    releaseName: "p2p-console client-v0.2.0",
    releaseNotesMd: "## notes\n- item",
    publishedAtMs: Date.parse("2026-09-01T10:00:00Z"),
    checkedAtMs: Date.now(),
  };
}

function upToDateResult(): UpdateCheckResult {
  return {
    ...availableResult(),
    latestVersion: "0.1.0",
    hasUpdate: false,
    releaseUrl: null,
    releaseName: null,
    releaseNotesMd: null,
    publishedAtMs: null,
  };
}

function resetStore(patch = {}): void {
  useUpdateStore.setState({
    status: "idle",
    result: null,
    error: null,
    lastSource: null,
    skippedVersion: null,
    reminderShownFor: null,
    ...patch,
  });
}

beforeEach(() => {
  localStorage.clear();
  updateCheckMock.mockReset();
  updateCheckMock.mockResolvedValue(upToDateResult());
  resetStore();
});

describe("AboutUpdateCard 手动检查三态", () => {
  it("idle：显示当前版本与可点的检查按钮", () => {
    render(<AboutUpdateCard />);
    expect(screen.getByText("当前版本")).toBeInTheDocument();
    expect(screen.getByText("v0.1.0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeEnabled();
  });

  it("检查中：按钮禁用且文案切换为检查中", () => {
    resetStore({ status: "checking" });
    render(<AboutUpdateCard />);
    expect(
      screen.getByRole("button", { name: "检查中…" }),
    ).toBeDisabled();
  });

  it("检查成功无更新：显示已是最新版本", async () => {
    render(<AboutUpdateCard />);
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
    await waitFor(() =>
      expect(screen.getByText("已是最新版本")).toBeInTheDocument(),
    );
    expect(updateCheckMock).toHaveBeenCalledTimes(1);
    expect(useUpdateStore.getState().lastSource).toBe("manual");
  });

  it("失败态可见可重试：重试成功回到已是最新", async () => {
    resetStore({ status: "failed", error: "网络不可达", lastSource: "manual" });
    render(<AboutUpdateCard />);
    expect(screen.getByText(/检查失败/)).toBeInTheDocument();
    expect(screen.getByText(/网络不可达/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() =>
      expect(screen.getByText("已是最新版本")).toBeInTheDocument(),
    );
    expect(useUpdateStore.getState().status).toBe("upToDate");
  });
});

describe("AboutUpdateCard 有更新详情", () => {
  it("展示新版本、发布名、说明与下载、跳过入口；跳过持久化", () => {
    resetStore({ status: "available", result: availableResult() });
    render(<AboutUpdateCard />);
    expect(screen.getByText("发现新版本 0.2.0")).toBeInTheDocument();
    expect(screen.getByText("p2p-console client-v0.2.0")).toBeInTheDocument();
    expect(screen.getByText("发布说明")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "跳过此版本" }));
    expect(localStorage.getItem("p2p-console.skipped-version")).toBe("0.2.0");
    expect(useUpdateStore.getState().skippedVersion).toBe("0.2.0");
  });

  it("超长说明截断并提示完整内容跳浏览器", () => {
    const longNotes = "# notes\n" + "x".repeat(500);
    resetStore({
      status: "available",
      result: { ...availableResult(), releaseNotesMd: longNotes },
    });
    render(<AboutUpdateCard />);
    expect(screen.getByText(/说明内容已截断/)).toBeInTheDocument();
  });

  it("已跳过时显示跳过提示，详情仍可在设置段查看", () => {
    resetStore({
      status: "available",
      result: availableResult(),
      skippedVersion: "0.2.0",
    });
    render(<AboutUpdateCard />);
    expect(screen.getByText(/已跳过版本 0\.2\.0/)).toBeInTheDocument();
    expect(screen.getByText("发现新版本 0.2.0")).toBeInTheDocument();
  });
});
