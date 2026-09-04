import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  GroupJson,
  NodeEventHandler,
  NodeStatus,
} from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";

// 建群流程（G3 验收）：选好友 → 命名 → 创建 → 群会话出现。
// groupCreate 返回值入列并选中，会话头渲染新群名。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    groupCreate: vi.fn<() => Promise<GroupJson>>(),
    nodeStatus: vi.fn<() => Promise<NodeStatus>>(),
    chatFriendsList: vi.fn<() => Promise<ChatFriendJson[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    groupCreate: mocks.groupCreate,
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
const ALICE = peerId("alice");
const BOB = peerId("bob");
const NEW_ID = "00000000-0000-4000-8000-000000000009";

function friend(peer: string, nickname: string): ChatFriendJson {
  return { peerId: peer, nickname, addrs: [], note: null };
}

beforeEach(() => {
  mocks.groupList.mockReset().mockResolvedValue([]);
  mocks.groupHistory.mockReset().mockResolvedValue([]);
  mocks.groupCreate.mockReset();
  mocks.chatFriendsList.mockReset().mockResolvedValue([
    friend(ALICE, "小爱"),
    friend(BOB, "小博"),
  ]);
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

describe("GroupCreateDialog 建群流程", () => {
  it("选好友 → 命名 → 创建 → 群会话出现且命令入参正确", async () => {
    mocks.groupCreate.mockResolvedValue({
      groupId: NEW_ID,
      name: "新项目组",
      owner: SELF,
      members: [SELF, ALICE, BOB],
      rev: 0,
      state: "active",
      tsMs: 2000,
    });

    render(
      <MemoryRouter>
        <GroupView />
      </MemoryRouter>,
    );
    fireEvent.click(await screen.findByTestId("group-create"));
    await waitFor(() =>
      expect(screen.getByTestId("group-create-dialog")).toBeTruthy(),
    );
    // 好友簿加载后渲染勾选项
    await waitFor(() =>
      expect(screen.getByTestId("group-create-friends").textContent).toContain("小爱"),
    );

    fireEvent.click(screen.getByTestId("group-create-friend-" + ALICE));
    fireEvent.click(screen.getByTestId("group-create-friend-" + BOB));
    fireEvent.change(screen.getByTestId("group-create-name"), {
      target: { value: "  新项目组  " },
    });
    fireEvent.click(screen.getByTestId("group-create-submit"));

    await waitFor(() => expect(mocks.groupCreate).toHaveBeenCalled());
    // 名单 trim + 去重 + 好友 PeerId 原样（不含本机）
    expect(mocks.groupCreate).toHaveBeenCalledWith("新项目组", [ALICE, BOB]);
    await waitFor(() =>
      expect(mocks.groupHistory).toHaveBeenCalledWith(NEW_ID, null, 50),
    );
    await waitFor(() =>
      expect(
        screen.getByTestId("group-conversation-header").textContent,
      ).toContain("新项目组"),
    );
    // 对话框关闭、群列表出现新群
    expect(screen.queryByTestId("group-create-dialog")).toBeNull();
    expect(screen.getAllByTestId("group-row")).toHaveLength(1);
  });

  it("未选好友或未命名时提交不可用；后端拒绝保留表单并展示原文", async () => {
    render(
      <MemoryRouter>
        <GroupView />
      </MemoryRouter>,
    );
    fireEvent.click(await screen.findByTestId("group-create"));
    await waitFor(() =>
      expect(screen.getByTestId("group-create-friends").textContent).toContain("小博"),
    );

    const submit = screen.getByTestId("group-create-submit") as HTMLButtonElement;
    expect(submit.disabled).toBe(true); // 未命名未选人
    fireEvent.change(screen.getByTestId("group-create-name"), {
      target: { value: "临时群" },
    });
    expect(submit.disabled).toBe(true); // 仍未选人

    fireEvent.click(screen.getByTestId("group-create-friend-" + ALICE));
    expect(submit.disabled).toBe(false);

    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    fireEvent.click(screen.getByTestId("group-create-friend-" + ALICE)); // 取消勾选
    fireEvent.click(screen.getByTestId("group-create-friend-" + BOB));
    mocks.groupCreate.mockRejectedValueOnce(new Error("建群命令失败"));
    fireEvent.click(submit);
    await waitFor(() =>
      expect(screen.getByTestId("group-create-error").textContent).toContain(
        "建群命令失败",
      ),
    );
    expect(screen.getByTestId("group-create-dialog")).toBeTruthy();
    errSpy.mockRestore();
  });
});
