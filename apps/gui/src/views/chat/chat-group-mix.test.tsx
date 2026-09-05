import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { HashRouter, Route, Routes, useLocation } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  GroupJson,
  NodeEventHandler,
} from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";

// 会话列表 1:1/群混排（G3 验收）：群分节置于好友分组之后，GroupJson.state
// 过滤排序（active 在前、非 active 置底），点击群行跳 /group?g=<id>。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    groupList: mocks.groupList,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { ChatView } from "./chat-view";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

const GROUP_A = "00000000-0000-4000-8000-00000000000a";
const GROUP_B = "00000000-0000-4000-8000-00000000000b";

function group(
  groupId: string,
  name: string,
  state: GroupJson["state"],
  tsMs: number,
): GroupJson {
  return {
    groupId,
    name,
    owner: peerId("owner"),
    members: [peerId("owner"), peerId("m1")],
    rev: 1,
    state,
    tsMs,
  };
}

function LocationProbe() {
  const location = useLocation();
  return (
    <span data-testid="location-probe">
      {location.pathname}
      {location.search}
    </span>
  );
}

// ChatView 经 window.location.hash 跳转（App 根为 HashRouter），测试同构。
function renderChat(): void {
  window.location.hash = "/chat";
  render(
    <HashRouter>
      <Routes>
        <Route path="/chat" element={<ChatView />} />
        <Route
          path="/group"
          element={
            <>
              <span data-testid="group-page">群聊页</span>
              <LocationProbe />
            </>
          }
        />
        <Route path="*" element={<LocationProbe />} />
      </Routes>
    </HashRouter>,
  );
}

beforeEach(() => {
  window.location.hash = "/chat";
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.groupList.mockReset().mockResolvedValue([]);
  useChatStore.setState({
    friends: [],
    friendsLoaded: false,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
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

afterEach(() => {
  window.location.hash = "";
});

describe("ChatView 会话列表群混排", () => {
  it("好友分组之后渲染群聊分节：群名/成员数可见", async () => {
    mocks.friends.mockResolvedValue([
      { peerId: peerId("f1"), nickname: "小圆", addrs: [], note: null },
    ]);
    mocks.groupList.mockResolvedValue([group(GROUP_A, "项目组", "active", 1000)]);
    renderChat();

    await waitFor(() =>
      expect(screen.getByTestId("chat-group-section")).toBeTruthy(),
    );
    expect(screen.getByText("小圆")).toBeTruthy();
    const section = screen.getByTestId("chat-group-section");
    expect(section.textContent).toContain("项目组");
    expect(section.textContent).toContain("2 名成员");
  });

  it("state 过滤置底：active 在前、left 置底（即使 left 更新）", async () => {
    mocks.groupList.mockResolvedValue([
      group(GROUP_B, "已退的群", "left", 9000),
      group(GROUP_A, "进行群", "active", 1000),
    ]);
    renderChat();

    await waitFor(() =>
      expect(screen.getByTestId("chat-group-section")).toBeTruthy(),
    );
    const rows = screen
      .getAllByTestId(/^chat-group-row-[0-9a-f-]+$/)
      .map((el) => el.dataset.testid);
    expect(rows).toEqual([
      "chat-group-row-" + GROUP_A,
      "chat-group-row-" + GROUP_B,
    ]);
    expect(screen.getByText("已退出")).toBeTruthy();
  });

  it("点击群行跳 /group?g=<id>", async () => {
    mocks.groupList.mockResolvedValue([group(GROUP_A, "项目组", "active", 1000)]);
    renderChat();

    const row = await screen.findByTestId("chat-group-row-" + GROUP_A);
    fireEvent.click(row);
    await waitFor(() =>
      expect(screen.getByTestId("group-page")).toBeTruthy(),
    );
    expect(screen.getByTestId("location-probe").textContent).toBe(
      "/group?g=" + GROUP_A,
    );
  });

  it("无好友但有群时不再显示空态，仅渲染群分节", async () => {
    mocks.groupList.mockResolvedValue([group(GROUP_A, "项目组", "active", 1000)]);
    renderChat();

    await waitFor(() =>
      expect(screen.getByTestId("chat-group-section")).toBeTruthy(),
    );
    expect(screen.queryByText("暂无好友")).toBeNull();
  });
});
