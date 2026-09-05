// 验收 2（§3.2 加好友 out/in 全流程）：in 向收件箱（红点计数、填备注
// 昵称接受/拒绝）与 out 向「待对方同意」条目撤回；表单校验/错误路径/
// 旅程断言见 chat-friend-add.test.tsx（沿用迁移）。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, FriendInviteJson, NodeEventHandler } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    invites: vi.fn<() => Promise<FriendInviteJson[]>>(),
    accept: vi.fn<(peerId: string, nickname: string) => Promise<unknown>>(),
    reject: vi.fn<(peerId: string) => Promise<void>>(),
    cancel: vi.fn<(peerId: string) => Promise<boolean>>(),
    history: vi.fn<() => Promise<unknown[]>>(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatInviteAccept: mocks.accept,
    chatInviteReject: mocks.reject,
    chatInviteCancel: mocks.cancel,
    chatHistory: mocks.history,
    onNodeEvent: (handler: NodeEventHandler) => {
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { FriendSection } from "@/views/contacts/friend-section";
import { InviteInbox } from "@/views/contacts/invite-inbox";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null };
}

function inInvite(): FriendInviteJson {
  return { peerId: PEER, nickname: "对方昵称", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true };
}

beforeEach(() => {
  localStorage.clear();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.accept.mockReset();
  mocks.reject.mockReset();
  mocks.cancel.mockReset().mockResolvedValue(true);
  useChatStore.setState({
    invites: [],
    friends: [],
    friendsLoaded: true,
    friendsError: null,
  });
});

describe("通讯录好友邀请全流程（§3.2，P2 验收 2）", () => {
  it("in 向邀请：页顶红点计数、展开收件箱、填备注昵称接受后入好友簿", async () => {
    mocks.friends.mockResolvedValue([]);
    useChatStore.setState({
      invites: [
        { peerId: PEER, nickname: "对方昵称", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true },
      ],
    });
    mocks.accept.mockResolvedValue(friendOf(PEER, "小圆"));
    render(
      <MemoryRouter>
        <FriendSection />
        <InviteInbox />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByTestId("contacts-invite-dot")).toBeTruthy());
    // 收件箱默认折叠：红点徽标计数可见，展开后出现逐条处理表单
    expect(screen.queryByTestId("contacts-invite-in-" + PEER)).toBeNull();
    fireEvent.click(screen.getByTestId("contacts-invite-toggle"));
    expect(screen.getByTestId("contacts-invite-in-" + PEER)).toBeTruthy();
    fireEvent.change(screen.getByTestId("contacts-invite-nickname-" + PEER), {
      target: { value: "小圆" },
    });
    fireEvent.click(screen.getByTestId("contacts-invite-accept-" + PEER));
    await waitFor(() => expect(mocks.accept).toHaveBeenCalledWith(PEER, "小圆"));
  });

  it("in 向邀请拒绝：不建好友，仅刷新邀请簿", async () => {
    mocks.friends.mockResolvedValue([]);
    useChatStore.setState({
      invites: [
        { peerId: PEER, nickname: "对方昵称", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true },
      ],
    });
    render(
      <MemoryRouter>
        <FriendSection />
        <InviteInbox />
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByTestId("contacts-invite-dot")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contacts-invite-toggle"));
    fireEvent.click(screen.getByTestId("contacts-invite-reject-" + PEER));
    await waitFor(() => expect(mocks.reject).toHaveBeenCalledWith(PEER));
    expect(mocks.accept).not.toHaveBeenCalled();
  });

  it("out 向邀请撤回：好友区「待对方同意」条目可撤回，撤回不建好友", async () => {
    mocks.friends.mockResolvedValue([]);
    mocks.invites.mockResolvedValue([
      { peerId: PEER, nickname: "小圆", addrs: [], note: null, direction: "out", tsMs: 1, delivered: true },
    ]);
    useChatStore.setState({
      invites: [
        { peerId: PEER, nickname: "小圆", addrs: [], note: null, direction: "out", tsMs: 1, delivered: true },
      ],
    });
    render(
      <MemoryRouter>
        <FriendSection />
        <InviteInbox />
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByTestId("contacts-invite-out-" + PEER)).toBeTruthy(),
    );
    fireEvent.click(screen.getByTestId("contacts-invite-cancel-" + PEER));
    await waitFor(() => expect(mocks.cancel).toHaveBeenCalledWith(PEER));
    expect(mocks.accept).not.toHaveBeenCalled();
  });
});

