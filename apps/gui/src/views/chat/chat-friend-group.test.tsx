import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, NodeEventHandler } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    updateFriend: vi.fn<
      (
        peerId: string,
        patch: { group?: string | null; nickname?: string | null; note?: string | null },
      ) => Promise<ChatFriendJson>
    >(),
    history: vi.fn<() => Promise<unknown[]>>(),
    send: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatFriendUpdate: mocks.updateFriend,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { ChatView } from "./chat-view";

// 真实 base58（解码恰 32 字节），与后端 parse_peer_id 同口径的合法夹具
const PEER_A = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR";
const PEER_C = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";

function friendOf(peerId: string, nickname: string, group?: string | null): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null, group: group ?? null };
}

function seedFriends(): ChatFriendJson[] {
  return [
    friendOf(PEER_A, "甲", "同事"),
    friendOf(PEER_B, "乙", "家人"),
    friendOf(PEER_C, "丙"),
  ];
}

beforeEach(() => {
  window.localStorage.clear();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.updateFriend.mockReset();
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
});

async function renderWithFriends() {
  mocks.friends.mockResolvedValue(seedFriends());
  render(<ChatView />);
  await waitFor(() => expect(screen.getByText("甲")).toBeTruthy());
}

describe("好友分组渲染（IM-T43）", () => {
  it("按组名分节渲染，未分组虚拟组置底", async () => {
    await renderWithFriends();
    const headers = screen
      .getAllByTestId(/^friend-group-header-/)
      .map((el) => el.textContent);
    expect(headers.length).toBe(3);
    // 组名字典序随 locale 实现而异，只机械断言未分组恒置底
    expect(headers[2]).toContain("未分组");
    expect(screen.getByTestId("friend-group-header-__ungrouped__")).toBeTruthy();
    const ungrouped = screen.getByTestId("friend-group-__ungrouped__");
    expect(within(ungrouped).getByText("丙")).toBeTruthy();
    const colleagues = screen.getByTestId("friend-group-同事");
    expect(within(colleagues).getByText("甲")).toBeTruthy();
  });

  it("组头折叠：点击后组内条目收起，再点恢复且折叠态持久", async () => {
    await renderWithFriends();
    const header = screen.getByTestId("friend-group-header-同事");
    expect(screen.getByText("甲")).toBeTruthy();
    fireEvent.click(header);
    await waitFor(() => expect(screen.queryByText("甲")).toBeNull());
    expect(header.getAttribute("aria-expanded")).toBe("false");
    expect(
      JSON.parse(window.localStorage.getItem("chat.friendGroups.collapsed") ?? "[]"),
    ).toContain("同事");
    fireEvent.click(header);
    await waitFor(() => expect(screen.getByText("甲")).toBeTruthy());
  });
});

describe("移动分组（IM-T43）", () => {
  it("行内入口开框，提交后调用 chatFriendUpdate 且列表归属更新", async () => {
    await renderWithFriends();
    mocks.updateFriend.mockResolvedValue(friendOf(PEER_C, "丙", "同事"));
    fireEvent.click(screen.getByTestId(`chat-move-friend-${PEER_C}`));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-dialog")).toBeTruthy(),
    );
    fireEvent.change(screen.getByTestId("friend-move-input"), {
      target: { value: "同事" },
    });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    await waitFor(() =>
      expect(mocks.updateFriend).toHaveBeenCalledWith(PEER_C, { group: "同事" }),
    );
    await waitFor(() => {
      const colleagues = screen.getByTestId("friend-group-同事");
      expect(within(colleagues).getByText("丙")).toBeTruthy();
    });
    // 丙是唯一未分组好友，移出后未分组节整体消失（空节不渲染）
    expect(screen.queryByTestId("friend-group-__ungrouped__")).toBeNull();
  });

  it("清空组名提交 = 移出分组，后端收到空串，条目回到未分组节", async () => {
    await renderWithFriends();
    mocks.updateFriend.mockResolvedValue(friendOf(PEER_A, "甲", null));
    fireEvent.click(screen.getByTestId(`chat-move-friend-${PEER_A}`));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-dialog")).toBeTruthy(),
    );
    const input = screen.getByTestId("friend-move-input") as HTMLInputElement;
    expect(input.value).toBe("同事");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    await waitFor(() =>
      expect(mocks.updateFriend).toHaveBeenCalledWith(PEER_A, { group: "" }),
    );
    await waitFor(() => {
      const ungrouped = screen.getByTestId("friend-group-__ungrouped__");
      expect(within(ungrouped).getByText("甲")).toBeTruthy();
    });
  });

  it("后端校验拒绝：错误原文上浮在框内，不吞不翻译", async () => {
    await renderWithFriends();
    mocks.updateFriend.mockRejectedValue(new Error("好友不在簿：ghost"));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    fireEvent.click(screen.getByTestId(`chat-move-friend-${PEER_A}`));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-dialog")).toBeTruthy(),
    );
    fireEvent.change(screen.getByTestId("friend-move-input"), {
      target: { value: "新组" },
    });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-error").textContent).toContain(
        "好友不在簿：ghost",
      ),
    );
    expect(logSpy).toHaveBeenCalledWith(
      "[chat] 移动分组失败",
      PEER_A,
      expect.any(Error),
    );
    logSpy.mockRestore();
  });

  it("前端预校验：超长组名拦截并提示，不触达后端", async () => {
    await renderWithFriends();
    fireEvent.click(screen.getByTestId(`chat-move-friend-${PEER_A}`));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-dialog")).toBeTruthy(),
    );
    fireEvent.change(screen.getByTestId("friend-move-input"), {
      target: { value: "组".repeat(33) },
    });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-move-invalid").textContent).toContain(
        "32 字符",
      ),
    );
    expect(mocks.updateFriend).not.toHaveBeenCalled();
  });
});
