import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { GroupJson, NodeEventHandler } from "@/lib/ipc-types";

// 群聊空页壳（G2 验收）：群列表 mock 渲染（空态+有数据态）、错误重试、
// 加载态。ipc 经 vi.mock 替身，文案走真实 i18n 资源（默认 zh-CN）。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { GroupView } from "./group-view";

function group(
  groupId: string,
  name: string,
  state: GroupJson["state"],
  memberCount: number,
): GroupJson {
  return {
    groupId,
    name,
    owner: "3xY9owner0000000000000000000000000000000000",
    members: Array.from({ length: memberCount }, (_, i) => `member-${i}`),
    rev: 1,
    state,
    tsMs: 1000,
  };
}

beforeEach(() => {
  mocks.groupList.mockReset().mockResolvedValue([]);
});

describe("GroupView 群列表渲染", () => {
  it("空态：无群时显示空态引导", async () => {
    mocks.groupList.mockResolvedValue([]);
    render(<GroupView />);
    await waitFor(() =>
      expect(screen.getByText("暂无群聊")).toBeTruthy(),
    );
    expect(screen.getByText("创建群聊后即可在此开始群聊")).toBeTruthy();
  });

  it("有数据态：渲染群名/成员数/群主/状态徽标", async () => {
    mocks.groupList.mockResolvedValue([
      group("g-active", "项目组", "active", 3),
      group("g-left", "老群", "left", 5),
    ]);
    render(<GroupView />);
    await waitFor(() => expect(screen.getByText("项目组")).toBeTruthy());

    expect(screen.getByText("老群")).toBeTruthy();
    expect(screen.getByText("3 名成员")).toBeTruthy();
    expect(screen.getByText("5 名成员")).toBeTruthy();
    expect(screen.getAllByText(/群主 3xY9/).length).toBe(2);
    expect(screen.getByText("进行中")).toBeTruthy();
    expect(screen.getByText("已退出")).toBeTruthy();
    expect(screen.getAllByTestId("group-row")).toHaveLength(2);
  });

  it("加载失败显示错误原文与刷新入口，重试成功恢复列表", async () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.groupList.mockRejectedValueOnce(new Error("group list boom"));
    render(<GroupView />);
    await waitFor(() =>
      expect(screen.getByText("group list boom")).toBeTruthy(),
    );
    expect(screen.getByRole("button", { name: "刷新" })).toBeTruthy();

    mocks.groupList.mockResolvedValue([group("g-ok", "恢复群", "active", 2)]);
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.getByText("恢复群")).toBeTruthy());
    errSpy.mockRestore();
  });

  it("加载中显示加载文案而非空态", () => {
    mocks.groupList.mockImplementation(() => new Promise(() => {}));
    render(<GroupView />);
    expect(screen.getByText("正在加载群列表…")).toBeTruthy();
  });
});
