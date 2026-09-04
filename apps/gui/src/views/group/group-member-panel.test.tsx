import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import type {
  ChatFriendJson,
  GroupJson,
  NodeEventHandler,
  NodeStatus,
} from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";

// 成员面板四操作（G3 验收）：邀请/移除/退群/解散的 mock 交互断言，
// 外加改名。确认流经 ConfirmProvider，命令断言核对 ipc 入参。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    groupInvite: vi.fn<(groupId: string, memberIds: string[]) => Promise<GroupJson>>(),
    groupKick: vi.fn<(groupId: string, memberId: string) => Promise<GroupJson>>(),
    groupLeave: vi.fn<(groupId: string) => Promise<GroupJson>>(),
    groupRename: vi.fn<(groupId: string, name: string) => Promise<GroupJson>>(),
    nodeStatus: vi.fn<() => Promise<NodeStatus>>(),
    chatFriendsList: vi.fn<() => Promise<ChatFriendJson[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    groupInvite: mocks.groupInvite,
    groupKick: mocks.groupKick,
    groupLeave: mocks.groupLeave,
    groupRename: mocks.groupRename,
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
const CAROL = peerId("carol");
const FOREIGN_OWNER = peerId("owner99");
const GROUP_ID = "00000000-0000-4000-8000-000000000001";
const FOREIGN_ID = "00000000-0000-4000-8000-000000000002";

const GROUP: GroupJson = {
  groupId: GROUP_ID,
  name: "项目组",
  owner: SELF,
  members: [SELF, ALICE, BOB],
  rev: 0,
  state: "active",
  tsMs: 1000,
};

function friend(peer: string, nickname: string): ChatFriendJson {
  return { peerId: peer, nickname, addrs: [], note: null };
}

function renderView(initialEntry = "/"): void {
  render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ConfirmProvider>
        <GroupView />
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

async function openPanel(): Promise<void> {
  fireEvent.click(await screen.findByTestId("group-manage"));
  await waitFor(() =>
    expect(screen.getByTestId("group-member-panel")).toBeTruthy(),
  );
}

beforeEach(() => {
  mocks.groupList.mockReset().mockResolvedValue([GROUP]);
  mocks.groupHistory.mockReset().mockResolvedValue([]);
  mocks.groupInvite.mockReset();
  mocks.groupKick.mockReset();
  mocks.groupLeave.mockReset();
  mocks.groupRename.mockReset();
  mocks.chatFriendsList.mockReset().mockResolvedValue([
    friend(ALICE, "小爱"),
    friend(BOB, ""),
    friend(CAROL, "小卡"),
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

describe("GroupMemberPanel 邀请与移除", () => {
  it("邀请：好友簿勾选候选（排除在群成员），提交调 groupInvite 并入列", async () => {
    const invited: GroupJson = {
      ...GROUP,
      rev: 1,
      members: [SELF, ALICE, BOB, CAROL],
    };
    mocks.groupInvite.mockResolvedValueOnce(invited);
    renderView("?g=" + GROUP_ID);
    await openPanel();

    fireEvent.click(screen.getByTestId("group-invite-open"));
    await waitFor(() =>
      expect(screen.getByTestId("group-invite-picker")).toBeTruthy(),
    );
    // 候选含 CAROL；在群成员不可勾选
    fireEvent.click(screen.getByTestId("group-invite-" + CAROL));
    expect(screen.queryByTestId("group-invite-" + ALICE)).toBeNull();

    fireEvent.click(screen.getByTestId("group-invite-submit"));
    await waitFor(() => expect(mocks.groupInvite).toHaveBeenCalled());
    expect(mocks.groupInvite).toHaveBeenCalledWith(GROUP_ID, [CAROL]);
    // 成功后 picker 收起，成员列表出现新成员
    await waitFor(() =>
      expect(screen.queryByTestId("group-invite-picker")).toBeNull(),
    );
    expect(screen.getByTestId("group-member-" + CAROL)).toBeTruthy();
  });

  it("移除：确认流后调 groupKick；不能移除自己/群主入口", async () => {
    mocks.groupKick.mockResolvedValueOnce({
      ...GROUP,
      rev: 1,
      members: [SELF, BOB],
    });
    renderView("?g=" + GROUP_ID);
    await openPanel();

    // 群主行无移除入口；成员行有
    expect(screen.queryByTestId("group-kick-" + SELF)).toBeNull();
    fireEvent.click(screen.getByTestId("group-kick-" + ALICE));
    fireEvent.click(await screen.findByRole("button", { name: "移除" }));

    await waitFor(() => expect(mocks.groupKick).toHaveBeenCalled());
    expect(mocks.groupKick).toHaveBeenCalledWith(GROUP_ID, ALICE);
    await waitFor(() =>
      expect(screen.queryByTestId("group-member-" + ALICE)).toBeNull(),
    );
  });
});

describe("GroupMemberPanel 退群与解散", () => {
  it("退群：非群主可见入口，确认后调 groupLeave 且会话转只读", async () => {
    mocks.groupList.mockResolvedValue([
      { ...GROUP, groupId: FOREIGN_ID, name: "别人的群", owner: FOREIGN_OWNER },
    ]);
    mocks.groupLeave.mockResolvedValueOnce({
      ...GROUP,
      groupId: FOREIGN_ID,
      owner: FOREIGN_OWNER,
      state: "left",
    });
    renderView("?g=" + FOREIGN_ID);
    await openPanel();

    // 非 owner：无解散/邀请/移除入口，有退群
    expect(screen.queryByTestId("group-disband")).toBeNull();
    expect(screen.queryByTestId("group-invite-open")).toBeNull();
    expect(screen.queryByTestId("group-rename-input")).toBeNull();
    fireEvent.click(screen.getByTestId("group-leave"));
    fireEvent.click(await screen.findByRole("button", { name: "退出" }));

    await waitFor(() => expect(mocks.groupLeave).toHaveBeenCalled());
    expect(mocks.groupLeave).toHaveBeenCalledWith(FOREIGN_ID);
    await waitFor(() =>
      expect(screen.getByTestId("group-panel-readonly")).toBeTruthy(),
    );
  });

  it("解散：owner 专属入口，确认后逐个 groupKick 全体其他成员", async () => {
    // 逐个移除语义：mock 基于当前 store 名单过滤，模拟后端 rev 收敛
    mocks.groupKick.mockImplementation(async (_groupId: string, memberId: string) => {
      const current =
        useGroupStore.getState().groups.find((g) => g.groupId === GROUP_ID) ?? GROUP;
      return {
        ...current,
        rev: current.rev + 1,
        members: current.members.filter((m) => m !== memberId),
      };
    });
    renderView("?g=" + GROUP_ID);
    await openPanel();

    fireEvent.click(screen.getByTestId("group-disband"));
    fireEvent.click(await screen.findByRole("button", { name: "解散" }));

    await waitFor(() => expect(mocks.groupKick).toHaveBeenCalledTimes(2));
    expect(mocks.groupKick).toHaveBeenCalledWith(GROUP_ID, ALICE);
    expect(mocks.groupKick).toHaveBeenCalledWith(GROUP_ID, BOB);
    // 名单只剩 owner；base58 字符集正则排除容器 testid（panel/list 含 l）
    await waitFor(() =>
      expect(
        screen.getAllByTestId(/^group-member-[1-9A-HJ-NP-Za-km-z]+$/),
      ).toHaveLength(1),
    );
    expect(screen.getByTestId("group-member-" + SELF)).toBeTruthy();
  });
});

describe("GroupMemberPanel 改名", () => {
  it("owner 改名：提交调 groupRename，入参为 trim 后新名", async () => {
    mocks.groupRename.mockResolvedValueOnce({ ...GROUP, name: "新研发群", rev: 1 });
    renderView("?g=" + GROUP_ID);
    await openPanel();

    const input = screen.getByTestId("group-rename-input") as HTMLInputElement;
    expect(input.value).toBe("项目组");
    fireEvent.change(input, { target: { value: "  新研发群  " } });
    fireEvent.click(screen.getByTestId("group-rename-save"));

    await waitFor(() => expect(mocks.groupRename).toHaveBeenCalled());
    expect(mocks.groupRename).toHaveBeenCalledWith(GROUP_ID, "新研发群");
  });
});
