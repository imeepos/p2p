import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  GroupJson,
  GroupMessageJson,
  GroupSendReport,
  NodeEventHandler,
  NodeStatus,
} from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { useNodeStore } from "@/stores/node-store";

// 群消息渲染与发送（G3 验收）：senderId 经好友簿解析昵称、本端判定
// senderId===本机 PeerId、acks 送达计数「已送达 k/n」、输入条走群 transport。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<GroupMessageJson[]>>(),
    groupSend: vi.fn<() => Promise<GroupSendReport>>(),
    nodeStatus: vi.fn<() => Promise<NodeStatus>>(),
    chatFriendsList: vi.fn<() => Promise<ChatFriendJson[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    groupSend: mocks.groupSend,
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
const GROUP_ID = "00000000-0000-4000-8000-000000000001";
const GROUP: GroupJson = {
  groupId: GROUP_ID,
  name: "项目组",
  owner: SELF,
  members: [SELF, ALICE, BOB],
  rev: 0,
  state: "active",
  tsMs: 1000,
};

function gmsg(
  id: string,
  senderId: string,
  text: string,
  patch: Partial<GroupMessageJson> = {},
): GroupMessageJson {
  return {
    id,
    groupId: GROUP_ID,
    senderId,
    kind: "text",
    tsMs: 1700000000000,
    text,
    media: null,
    status: "delivered",
    acks: [],
    ...patch,
  };
}

function friend(peer: string, nickname: string): ChatFriendJson {
  return { peerId: peer, nickname, addrs: [], note: null };
}

async function renderWithGroup(): Promise<void> {
  render(
    <MemoryRouter initialEntries={[{ pathname: "/", search: "?g=" + GROUP_ID }]}>
      <GroupView />
    </MemoryRouter>,
  );
  await waitFor(() =>
    expect(
      screen.getByTestId("group-conversation-header").textContent,
    ).toContain("项目组"),
  );
}

beforeEach(() => {
  mocks.groupList.mockReset().mockResolvedValue([GROUP]);
  mocks.groupHistory.mockReset().mockResolvedValue([]);
  mocks.groupSend.mockReset();
  mocks.chatFriendsList.mockReset().mockResolvedValue([
    friend(ALICE, "小爱"),
    friend(BOB, ""),
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
  useNodeStore.setState({ status: { running: true, peerId: SELF } as NodeStatus });
});

describe("GroupConversation 消息渲染", () => {
  it("senderId 经好友簿解析昵称；未在册回退 PeerId 缩略", async () => {
    mocks.groupHistory.mockResolvedValue([
      gmsg("m1", ALICE, "大家好"),
      gmsg("m2", peerId("stranger"), "路人消息"),
    ]);
    await renderWithGroup();

    const labels = screen.getAllByTestId("group-sender-label");
    expect(labels.map((el) => el.textContent)).toEqual([
      "小爱",
      peerId("stranger").slice(0, 8),
    ]);
    expect(screen.getByText("大家好")).toBeTruthy();
  });

  it("本端消息靠右（senderId===本机）且 acks 计数展示「已送达 k/n」", async () => {
    mocks.groupHistory.mockResolvedValue([
      gmsg("mine", SELF, "我发的", { status: "pending", acks: [ALICE] }),
      gmsg("theirs", BOB, "别人发的"),
    ]);
    await renderWithGroup();

    // n = 3 名成员 - 本机 = 2；acks = 1 → 已送达 1/2；them 消息无计数
    expect(screen.getByText("已送达 1/2")).toBeTruthy();
    expect(screen.getAllByTestId("group-sender-label")).toHaveLength(1);
    expect(screen.getAllByTestId("message-status")).toHaveLength(1);
  });

  it("非 active 群只读：只读横幅出现，输入条禁用", async () => {
    mocks.groupList.mockResolvedValue([{ ...GROUP, state: "left" }]);
    await renderWithGroup();

    expect(screen.getByTestId("group-readonly-hint").textContent).toContain(
      "已退出",
    );
    expect(
      (screen.getByTestId("group-input") as HTMLTextAreaElement).disabled,
    ).toBe(true);
  });
});

describe("GroupConversation 发送", () => {
  it("输入条经群 transport 发送：groupSend 收到裁剪文本，真身气泡替换占位", async () => {
    const real = gmsg("real-1", SELF, "你好群", { status: "pending" });
    mocks.groupSend.mockResolvedValue({
      message: real,
      acked: 0,
      recipients: 2,
      delivered: false,
    });
    await renderWithGroup();

    const input = screen.getByTestId("group-input");
    fireEvent.change(input, { target: { value: "  你好群  " } });
    fireEvent.click(screen.getByTestId("group-send"));

    await waitFor(() => expect(mocks.groupSend).toHaveBeenCalled());
    expect(mocks.groupSend).toHaveBeenCalledWith(
      GROUP_ID,
      "text",
      "你好群",
      undefined,
      undefined,
    );
    await waitFor(() => expect(screen.getByText("已送达 0/2")).toBeTruthy());
    expect(screen.getByText("你好群")).toBeTruthy();
  });
});
