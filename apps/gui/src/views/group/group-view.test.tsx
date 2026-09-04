import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  GroupJson,
  NodeEventHandler,
  NodeStatus,
} from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";

// 群聊页（G3）：列表渲染（空态/数据/错误/加载）、state 置底排序、
// URL ?g= 预选会话。ipc 经 vi.mock 替身，文案走真实 i18n（默认 zh-CN）。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    nodeStatus: vi.fn<() => Promise<NodeStatus>>(),
    chatFriendsList: vi.fn<() => Promise<never[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    nodeStatus: mocks.nodeStatus,
    chatFriendsList: mocks.chatFriendsList,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { GroupView } from "./group-view";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

const SELF = peerId("self");

function group(
  groupId: string,
  name: string,
  state: GroupJson["state"],
  memberCount: number,
  tsMs = 1000,
): GroupJson {
  return {
    groupId,
    name,
    owner: SELF,
    members: Array.from({ length: memberCount }, (_, i) => `member-${i}`),
    rev: 1,
    state,
    tsMs,
  };
}

function renderView(initialEntry = "/"): ReturnType<typeof render> {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <GroupView />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.groupList.mockReset().mockResolvedValue([]);
  mocks.groupHistory.mockReset().mockResolvedValue([]);
  mocks.chatFriendsList.mockReset().mockResolvedValue([]);
  mocks.nodeStatus.mockReset().mockResolvedValue({
    running: true,
    peerId: SELF,
  } as NodeStatus);
  useGroupStore.setState({
    groups: [],
    groupsLoaded: false,
    groupsError: null,
    friends: [],
    friendsLoaded: false,
    selfPeerId: null,
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
});

describe("GroupView 群列表渲染", () => {
  it("空态：无群时显示空态引导", async () => {
    renderView();
    await waitFor(() => expect(screen.getByText("暂无群聊")).toBeTruthy());
    expect(screen.getByText("创建群聊后即可在此开始群聊")).toBeTruthy();
  });

  it("有数据态：active 在前，非 active 置底且带状态徽标", async () => {
    mocks.groupList.mockResolvedValue([
      group("g-left", "老群", "left", 5, 3000),
      group("g-active", "项目组", "active", 3, 1000),
    ]);
    renderView();
    await waitFor(() => expect(screen.getByText("项目组")).toBeTruthy());

    expect(screen.getByText("老群")).toBeTruthy();
    expect(screen.getByText("3 名成员")).toBeTruthy();
    expect(screen.getByText("5 名成员")).toBeTruthy();
    expect(screen.getByText("已退出")).toBeTruthy();
    expect(screen.getAllByTestId("group-row")).toHaveLength(2);
    // 置底：行序 active → left
    const rows = screen.getAllByTestId("group-row");
    expect(rows[0]!.textContent).toContain("项目组");
    expect(rows[1]!.textContent).toContain("老群");
  });

  it("加载失败显示错误原文与刷新入口，重试成功恢复列表", async () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.groupList.mockRejectedValueOnce(new Error("group list boom"));
    renderView();
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
    renderView();
    expect(screen.getByText("正在加载群列表…")).toBeTruthy();
  });
});

describe("GroupView 会话选择", () => {
  it("点击群行加载历史并渲染会话头", async () => {
    mocks.groupList.mockResolvedValue([group("g-active", "项目组", "active", 3)]);
    mocks.groupHistory.mockResolvedValue([]);
    renderView();
    await waitFor(() => expect(screen.getByText("项目组")).toBeTruthy());
    fireEvent.click(screen.getAllByTestId("group-row")[0]!);
    await waitFor(() =>
      expect(mocks.groupHistory).toHaveBeenCalledWith("g-active", null, 50),
    );
    expect(screen.getByTestId("group-conversation-header").textContent).toContain(
      "项目组",
    );
  });

  it("URL ?g= 预选：挂载即选中指定群", async () => {
    mocks.groupList.mockResolvedValue([
      group("g-a", "甲群", "active", 2),
      group("g-b", "乙群", "active", 2),
    ]);
    mocks.groupHistory.mockResolvedValue([]);
    renderView("/?g=g-b");
    await waitFor(() =>
      expect(mocks.groupHistory).toHaveBeenCalledWith("g-b", null, 50),
    );
    await waitFor(() =>
      expect(
        screen.getByTestId("group-conversation-header").textContent,
      ).toContain("乙群"),
    );
  });

  it("未选群时会话区显示空态引导", async () => {
    mocks.groupList.mockResolvedValue([group("g-a", "甲群", "active", 2)]);
    renderView();
    await waitFor(() => expect(screen.getByText("选择群聊查看消息")).toBeTruthy());
  });
});
