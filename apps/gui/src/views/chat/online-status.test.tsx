import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  NodeEventJson,
  NodeEventHandler,
} from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<unknown[]>
    >(),
    send: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { reduceEvent, type EventStateSlice } from "@/stores/event-reducer";
import { useChatStore } from "@/stores/chat-store";
import { useNodeStore } from "@/stores/node-store";
import { ChatView } from "./chat-view";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

function friend(seed: string, nickname: string): ChatFriendJson {
  return { peerId: peerId(seed), nickname, addrs: [], note: null };
}

const PEER_A = peerId("friend-a");
const PEER_B = peerId("friend-b");

// 与生产同口径：事件经 event-reducer 流入 node-store（peers[peer].connected 翻转）。
function applyPeerEvents(events: NodeEventJson[]): void {
  act(() => {
    useNodeStore.setState((s) => {
      const slice = events.reduce<EventStateSlice>(
        (acc, event) => reduceEvent(acc, event),
        { events: s.events, peers: s.peers, status: s.status },
      );
      return { peers: slice.peers, events: slice.events };
    });
  });
}

function rowStatus(peer: string): HTMLElement {
  return screen.getByTestId(`chat-peer-status-${peer}`);
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.send.mockReset();
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
  });
  act(() => {
    useNodeStore.setState({ peers: {}, events: [], status: null });
  });
});

describe("好友在线状态", () => {
  it("事件驱动切换：在线置绿、离线置灰，行标识与会话头同步翻转", async () => {
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    fireEvent.click(screen.getByText("小圆"));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());

    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("false");
    expect(screen.getByText("离线")).toBeTruthy();

    applyPeerEvents([{ type: "peer_connected", peer: PEER_A }]);
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("true");
    expect(screen.getByTestId("chat-header-status").getAttribute("data-online")).toBe("true");
    expect(screen.getByText("在线")).toBeTruthy();

    applyPeerEvents([{ type: "peer_disconnected", peer: PEER_A }]);
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("false");
    expect(screen.getByTestId("chat-header-status").getAttribute("data-online")).toBe("false");
    expect(screen.getByText("离线")).toBeTruthy();
  });

  it("刷新后初始态与事件流一致：挂载前已连接的好友首帧即在线", async () => {
    applyPeerEvents([{ type: "peer_connected", peer: PEER_A }]);
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    render(<ChatView />);
    await waitFor(() => expect(rowStatus(PEER_A)).toBeTruthy());
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("true");
  });

  it("多好友混合状态：在线与离线并存且互不串扰", async () => {
    applyPeerEvents([{ type: "peer_connected", peer: PEER_A }]);
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆"), friend("friend-b", "阿直")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("阿直")).toBeTruthy());
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("true");
    expect(rowStatus(PEER_B).getAttribute("data-online")).toBe("false");
    applyPeerEvents([{ type: "peer_connected", peer: PEER_B }]);
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("true");
    expect(rowStatus(PEER_B).getAttribute("data-online")).toBe("true");
  });

  it("节点未启动：无任何 peer 事件时全部离线态，渲染不异常", async () => {
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆"), friend("friend-b", "阿直")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    expect(rowStatus(PEER_A).getAttribute("data-online")).toBe("false");
    expect(rowStatus(PEER_B).getAttribute("data-online")).toBe("false");
    expect(screen.getByText("暂无会话，选择好友开始聊天")).toBeTruthy();
  });

  it("状态标识不干扰点击选择：直接点击在线点所在行仍触发选会话", async () => {
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    mocks.history.mockResolvedValue([]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    fireEvent.click(rowStatus(PEER_A));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
    expect(mocks.history).toHaveBeenCalledWith(PEER_A, null, 50);
  });
});
