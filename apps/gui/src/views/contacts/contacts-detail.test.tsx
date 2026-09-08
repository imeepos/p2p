// 双栏改版验收：资料卡默认选中/行点选联动/全局检索过滤/资料卡内管理流。
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, NodeEventHandler } from "@/lib/ipc-types";

const handlers: NodeEventHandler[] = [];
const mocks = vi.hoisted(() => ({
  friends: vi.fn(),
  invites: vi.fn(),
  history: vi.fn(),
  groupList: vi.fn(),
  nodeStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatHistory: mocks.history,
    chatFriendInvite: vi.fn(),
    chatFriendRemove: vi.fn(),
    chatFriendUpdate: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatInviteCancel: vi.fn(),
    chatSend: vi.fn(),
    groupList: mocks.groupList,
    groupCreate: vi.fn(),
    groupInvite: vi.fn(),
    groupLeave: vi.fn(),
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { ContactsPage } from "@/routes/contacts-page";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "2jSUsWcEf7z68xBscf2YmVYzQ4uPZfpMz8XRW3vruJU4";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: "你好", group: null };
}

function renderContacts(initialEntry = "/contacts") {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ContactsPage />
    </MemoryRouter>,
  );
}

function detailPane() {
  return within(screen.getByTestId("contacts-detail-pane"));
}

beforeEach(() => {
  handlers.length = 0;
  localStorage.clear();
  useEndpointMetaStore.getState().resetForTest();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.groupList.mockReset().mockResolvedValue([]);
  mocks.nodeStatus.mockReset().mockResolvedValue({
    running: true,
    peerId: PEER,
    listenAddrs: [],
    uptimeSecs: 1,
    startedAtMs: 1,
    config: {},
  });
  useChatStore.setState({
    invites: [],
    friends: [],
    friendsLoaded: false,
    friendsError: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    unreadByPeer: {},
  });
  useGroupStore.setState({
    groups: [],
    groupsLoaded: false,
    groupsError: null,
    selfPeerId: PEER,
    friends: [],
    friendsLoaded: false,
  });
  useAcpStore.setState({ saved: [], activeEndpointId: null, phase: "idle" });
  Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
});

describe("通讯录双栏资料卡", () => {
  it("空数据：右栏空态引导，无资料卡动作", () => {
    renderContacts();
    expect(detailPane().getByText("选择一个联系人")).toBeTruthy();
    expect(screen.queryByTestId("contacts-detail-move")).toBeNull();
  });

  it("默认选中首个好友：资料卡展示昵称/备注/ID，发消息深链 /chat?peer=", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    renderContacts();
    await waitFor(() => expect(detailPane().getByText("小圆")).toBeTruthy());
    expect(detailPane().getByText("你好")).toBeTruthy();
    expect(detailPane().getByText(PEER)).toBeTruthy();
    expect(detailPane().getByTestId("contacts-detail-message").getAttribute("href")).toBe(
      "/chat?peer=" + PEER,
    );
    // 默认选中即高亮对应行
    const row = screen.getByTestId("contact-friend-" + PEER);
    expect(row.classList.contains("bg-accent")).toBe(true);
  });

  it("点选行切换资料卡：第二位好友的昵称与发消息深链随之切换", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆"), friendOf(PEER_B, "小乙")]);
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contact-friend-" + PEER_B)).toBeTruthy());
    const rowB = screen.getByTestId("contact-friend-" + PEER_B);
    // 行内首钮即选中钮（其余为悬停显隐的动作钮）
    fireEvent.click(within(rowB).getAllByRole("button")[0]);
    await waitFor(() =>
      expect(detailPane().getByTestId("contacts-detail-message").getAttribute("href")).toBe(
        "/chat?peer=" + PEER_B,
      ),
    );
    expect(rowB.classList.contains("bg-accent")).toBe(true);
    expect(screen.getByTestId("contact-friend-" + PEER).classList.contains("bg-accent")).toBe(false);
  });

  it("全局检索过滤行项：命中仅留匹配好友，计数同步 matched/total", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆"), friendOf(PEER_B, "小乙")]);
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contact-friend-" + PEER_B)).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-search-global"), {
      target: { value: "小乙" },
    });
    expect(screen.queryByTestId("contact-friend-" + PEER)).toBeNull();
    expect(screen.getByTestId("contact-friend-" + PEER_B)).toBeTruthy();
    expect(screen.getByTestId("contacts-count-friends").textContent).toBe("1/2");
  });
});

describe("好友资料编辑（IM-T43 消费面）", () => {
  it("编辑入口改显示名与备注：保存走 chat_friend_update 并回填资料卡", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    vi.mocked(ipc.chatFriendUpdate).mockResolvedValue({
      peerId: PEER,
      nickname: "圆圆",
      addrs: [],
      note: "同事",
      group: null,
    });
    renderContacts();
    await waitFor(() => expect(detailPane().getByText("小圆")).toBeTruthy());
    fireEvent.click(detailPane().getByTestId("contacts-detail-edit"));
    const dialog = screen.getByTestId("friend-edit-dialog");
    const nick = within(dialog).getByTestId("friend-edit-nickname") as HTMLInputElement;
    const note = within(dialog).getByTestId("friend-edit-note") as HTMLTextAreaElement;
    expect(nick.value).toBe("小圆");
    expect(note.value).toBe("你好");
    fireEvent.change(nick, { target: { value: "圆圆" } });
    fireEvent.change(note, { target: { value: "同事" } });
    fireEvent.click(within(dialog).getByTestId("friend-edit-submit"));
    await waitFor(() =>
      expect(vi.mocked(ipc.chatFriendUpdate).mock.calls[0]?.[0]).toBe(PEER),
    );
    const patch = vi.mocked(ipc.chatFriendUpdate).mock.calls[0]?.[1];
    expect(patch?.nickname).toBe("圆圆");
    expect(patch?.note).toBe("同事");
    await waitFor(() => expect(detailPane().getByText("圆圆")).toBeTruthy());
    expect(detailPane().getByText("同事")).toBeTruthy();
  });

  it("清空显示名提交：后端回退 PeerId 缩略语义，回填用返回值昵称", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    vi.mocked(ipc.chatFriendUpdate).mockResolvedValue({
      peerId: PEER,
      nickname: PEER.slice(0, 8),
      addrs: [],
      note: null,
      group: null,
    });
    renderContacts();
    await waitFor(() => expect(detailPane().getByText("小圆")).toBeTruthy());
    fireEvent.click(detailPane().getByTestId("contacts-detail-remark-edit"));
    const dialog = screen.getByTestId("friend-edit-dialog");
    fireEvent.change(within(dialog).getByTestId("friend-edit-nickname"), {
      target: { value: "" },
    });
    fireEvent.click(within(dialog).getByTestId("friend-edit-submit"));
    await waitFor(() =>
      expect(vi.mocked(ipc.chatFriendUpdate).mock.calls[0]?.[0]).toBe(PEER),
    );
    await waitFor(() =>
      expect(detailPane().getByText(PEER.slice(0, 8))).toBeTruthy(),
    );
    // 备注清空后显示「未设置」占位
    expect(detailPane().getByText("未设置")).toBeTruthy();
  });
});
