// 验收 1（§3.1）：三区渲染——空态与行内操作断言；锚点条定位与当前节
// 高亮断言；/contacts#* 深链定位断言（5.2 命令面板锚点契约）。
import { fireEvent, render, screen, waitFor, type RenderResult } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  GroupJson,
  NodeEventHandler,
} from "@/lib/ipc-types";

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
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { ContactsPage } from "@/routes/contacts-page";

// 真实 base58（解码恰 32 字节）夹具
export const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "2jSUsWcEf7z68xBscf2YmVYzQ4uPZfpMz8XRW3vruJU4";

export function friendOf(peerId: string, nickname: string, group?: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null, group: group ?? null };
}

export function groupOf(groupId: string, name: string, owner: string): GroupJson {
  return {
    groupId,
    name,
    owner,
    members: [owner],
    rev: 1,
    state: "active",
    tsMs: 1_000,
  };
}

export function renderContacts(initialEntry = "/contacts"): RenderResult {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ContactsPage />
    </MemoryRouter>,
  );
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

describe("通讯录三区渲染（§3.1）", () => {
  it("空态：三分节与页顶锚点条就位，各区空态文案与添加引导可见", async () => {
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contacts-section-friends")).toBeTruthy());
    expect(screen.getByTestId("contacts-section-groups")).toBeTruthy();
    expect(screen.getByTestId("contacts-section-agents")).toBeTruthy();
    // 锚点条恰三枚按钮，id 契约与命令面板 /contacts#* 一致
    for (const id of ["friends", "groups", "agents"]) {
      expect(document.getElementById(id), "缺分节锚点: " + id).not.toBeNull();
      expect(screen.getByTestId("contacts-anchor-" + id)).toBeTruthy();
    }
    expect(screen.getByText("还没有好友")).toBeTruthy();
    expect(screen.getByText("还没有群聊")).toBeTruthy();
    expect(screen.getByText("还没有 Agent")).toBeTruthy();
    // 各节标题右侧「添加」按钮常驻
    expect(screen.getByTestId("contacts-friend-add")).toBeTruthy();
    expect(screen.getByTestId("contacts-group-add")).toBeTruthy();
    expect(screen.getByTestId("contacts-agent-add")).toBeTruthy();
  });

  it("好友区行内操作：发消息（/chat?peer=）、移动分组、删除均可达", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆", "家人")]);
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contact-friend-" + PEER)).toBeTruthy());
    const message = screen.getByTestId("contact-friend-message-" + PEER);
    expect(message.getAttribute("href")).toBe("/chat?peer=" + PEER);
    expect(screen.getByTestId("contact-friend-move-" + PEER)).toBeTruthy();
    expect(screen.getByTestId("contact-friend-remove-" + PEER)).toBeTruthy();
    // 分组名随行展示
    expect(screen.getByTestId("contact-friend-" + PEER).textContent).toContain("家人");
  });

  it("群区行模型：群名 + 成员数 + 我的角色；非 owner 邀请禁用；被踢群不进通讯录", async () => {
    const mine = groupOf("g-mine", "我的群", PEER);
    const joined = { ...groupOf("g-other", "别人的群", PEER_B), members: [PEER_B, PEER] };
    const kicked = { ...groupOf("g-kicked", "被踢的群", PEER_B), members: [PEER_B], state: "kicked" as const };
    mocks.groupList.mockResolvedValue([joined, kicked, mine]);
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contact-group-g-mine")).toBeTruthy());
    expect(screen.getByTestId("contact-group-g-other").textContent).toContain("成员");
    expect(screen.getByTestId("contact-group-g-mine").textContent).toContain("群主");
    // 非 active（kicked）不进通讯录：不渲染对应行
    expect(screen.queryByTestId("contact-group-g-kicked")).toBeNull();
    // 非 owner：邀请成员禁用；active：退群可用
    expect(
      (screen.getByTestId("contact-group-invite-g-other") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("contact-group-leave-g-other") as HTMLButtonElement).disabled,
    ).toBe(false);
    // 消息深链
    expect(screen.getByTestId("contact-group-message-g-mine").getAttribute("href")).toBe(
      "/chat?group=g-mine",
    );
  });

  it("Agent 区行模型：别名 + host + 连接态 + 权限档摘要 + 行内操作", async () => {
    useAcpStore.setState({
      saved: [
        {
          wsUrl: "ws://127.0.0.1:8787",
          token: "t",
          peer: PEER_B,
          alias: "编码助手",
          endpointId: "ep-1",
        },
      ],
    });
    useEndpointMetaStore.getState().setPolicy("ep-1", {
      defaults: { read: "allow", execute: "deny" },
      exceptions: [],
    });
    renderContacts();
    await waitFor(() => expect(screen.getByTestId("contact-agent-ep-1")).toBeTruthy());
    const row = screen.getByTestId("contact-agent-ep-1");
    expect(row.textContent).toContain("编码助手");
    expect(row.textContent).toContain("127.0.0.1:8787");
    expect(row.textContent).toContain("自动1");
    expect(screen.getByTestId("contact-agent-message-ep-1").getAttribute("href")).toBe(
      "/chat?agent=ep-1",
    );
    expect(screen.getByTestId("contact-agent-detail-ep-1")).toBeTruthy();
    expect(screen.getByTestId("contact-agent-disable-ep-1")).toBeTruthy();
    expect(screen.getByTestId("contact-agent-remove-ep-1")).toBeTruthy();
  });

  it("锚点条定位与当前节高亮；/contacts#groups 深链落定即定位", async () => {
    renderContacts("/contacts#groups");
    await waitFor(() => {
      expect(
        screen.getByTestId("contacts-anchor-groups").getAttribute("data-active"),
      ).toBe("true");
    });
    const scrollIntoView = vi.fn();
    Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoView,
    });
    fireEvent.click(screen.getByTestId("contacts-anchor-agents"));
    expect(scrollIntoView).toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.getByTestId("contacts-anchor-agents").getAttribute("data-active")).toBe("true");
      expect(screen.getByTestId("contacts-anchor-groups").getAttribute("data-active")).toBe("false");
    });
  });
});
