// 验收 3（§3.2 入群/邀请 + §3.5）：创建群 → 邀请好友 → 受邀方入群全流程
// 断言（roster 推送落群区）；32 上限前端拦截与后端拒绝路径。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  groupCreate: vi.fn(),
  groupInvite: vi.fn(),
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
    groupCreate: mocks.groupCreate,
    groupInvite: mocks.groupInvite,
    groupLeave: vi.fn(),
    groupKick: vi.fn(),
    groupRename: vi.fn(),
    groupDisband: vi.fn(),
    groupHistory: vi.fn().mockResolvedValue([]),
    groupSend: vi.fn(),
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { GroupSection } from "./group-section";

export const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "2jSUsWcEf7z68xBscf2YmVYzQ4uPZfpMz8XRW3vruJU4";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null, group: null };
}

function groupOf(groupId: string, name: string, owner: string, members: string[]): GroupJson {
  return { groupId, name, owner, members, rev: 1, state: "active", tsMs: 1_000 };
}

function renderSection() {
  return render(
    <MemoryRouter>
      <ConfirmProvider>
        <GroupSection />
      </ConfirmProvider>
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
  mocks.groupCreate.mockReset();
  mocks.groupInvite.mockReset();
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
    friendsLoaded: true,
    friendsError: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    unreadByPeer: {},
  });
  useGroupStore.setState({
    groups: [],
    groupsLoaded: true,
    groupsError: null,
    selfPeerId: PEER,
    friends: [],
    friendsLoaded: true,
    selectedGroupId: null,
    unreadByGroup: {},
  });
});

describe("创建群 → 邀请 → 入群流程（P2 验收 3）", () => {
  it("创建群：填群名与成员后成为 owner，群出现在群区", async () => {
    const created = groupOf("g-1", "新群", PEER, [PEER, PEER_B]);
    mocks.groupCreate.mockResolvedValue(created);
    useGroupStore.setState({ friends: [friendOf(PEER_B, "小圆")], selfPeerId: PEER });
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-group-add"));
    // F09：打开「添加群聊」默认页签即建群表单，一跳直达（无二选一中转）
    await waitFor(() => expect(screen.getByTestId("group-create-name")).toBeTruthy());
    fireEvent.change(screen.getByTestId("group-create-name"), { target: { value: "新群" } });
    // F23:成员选择为统一多选选择器,选项行按昵称定位(role=option)
    fireEvent.click(screen.getByRole("option", { name: /小圆/ }));
    fireEvent.click(screen.getByTestId("group-create-submit"));
    await waitFor(() => expect(mocks.groupCreate).toHaveBeenCalledWith("新群", [PEER_B]));
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    expect(screen.getByTestId("contact-group-g-1").textContent).toContain("群主");
  });

  it("F09：无好友时建群页签给「先去添加好友」引导而非死表单", async () => {
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-group-add"));
    await waitFor(() =>
      expect(screen.getByTestId("group-create-add-friend")).toBeTruthy(),
    );
    expect(screen.getByTestId("group-create-add-friend").textContent).toContain("先去添加好友");
  });

  it("F10：群名输入占位符说用户语言，不再泄漏 trim 内部术语", async () => {
    useGroupStore.setState({ friends: [friendOf(PEER_B, "小圆")], selfPeerId: PEER });
    renderSection();
    fireEvent.click(screen.getByTestId("contacts-group-add"));
    const input = await screen.findByTestId("group-create-name");
    expect(input.getAttribute("placeholder")).toBe("输入群名（1-64 个字）");
    expect(input.getAttribute("placeholder")).not.toContain("trim");
  });

  it("owner 邀请成员：好友多选发出邀请，roster 推送后受邀方群区出现该群（入群）", async () => {
    const mine = groupOf("g-1", "我的群", PEER, [PEER]);
    const invited = { ...mine, members: [PEER, PEER_B], rev: 2 };
    mocks.groupList.mockResolvedValue([mine]);
    useGroupStore.setState({
      groups: [mine],
      friends: [friendOf(PEER_B, "小圆")],
    });
    mocks.groupInvite.mockResolvedValue(invited);
    renderSection();
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-group-invite-g-1"));
    await waitFor(() => expect(screen.getByTestId("group-invite-picker")).toBeTruthy());
    fireEvent.click(await screen.findByRole("option", { name: /小圆/ }));
    fireEvent.click(screen.getByTestId("group-invite-submit"));
    await waitFor(() => expect(mocks.groupInvite).toHaveBeenCalledWith("g-1", [PEER_B]));
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1").textContent).toContain("2 名成员"));

    // 受邀方视角：chat_group_state roster 推送（新群含本机）→ 群区出现（入群）
    const theirs = { ...groupOf("g-1", "我的群", PEER, [PEER, PEER_B]), tsMs: 2_000 };
    for (const handler of handlers) {
      handler({ type: "chat_group_state", group: theirs });
    }
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1").textContent).toContain("2 名成员"));
  });

  it("32 上限：前端前置拦截（禁提交 + 提示），后端拒绝原文上浮", async () => {
    // 群现有 31 人（含 owner），再选 2 人即越界
    const members = Array.from({ length: 31 }, (_, i) => "peer" + String(i).padStart(2, "0"));
    const mine = groupOf("g-1", "大群", PEER, members);
    mocks.groupList.mockResolvedValue([mine]);
    const manyFriends = Array.from({ length: 5 }, (_, i) => friendOf("extra" + i, "好友" + i));
    useGroupStore.setState({ groups: [mine], friends: manyFriends });
    renderSection();
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-group-invite-g-1"));
    await waitFor(() => expect(screen.getByTestId("group-invite-picker")).toBeTruthy());
    // 31 + 1 = 32 恰好不越界
    fireEvent.click(await screen.findByRole("option", { name: /好友0/ }));
    await waitFor(() =>
      expect(screen.queryByTestId("group-invite-overcap")).toBeNull(),
    );
    // 再选 1 人 → 33 > 32：提示出现且提交被禁（不发无效请求）
    fireEvent.click(await screen.findByRole("option", { name: /好友1/ }));
    await waitFor(() => expect(screen.getByTestId("group-invite-overcap")).toBeTruthy());
    expect(
      (screen.getByTestId("group-invite-submit") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(mocks.groupInvite).not.toHaveBeenCalled();
  });

  it("后端拒绝（≤32 校验竞态/不在好友簿）：原文上浮不吞", async () => {
    const mine = groupOf("g-1", "群", PEER, [PEER]);
    mocks.groupList.mockResolvedValue([mine]);
    useGroupStore.setState({ groups: [mine], friends: [friendOf(PEER_B, "小圆")] });
    mocks.groupInvite.mockRejectedValue(new Error("成员数超上限"));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    renderSection();
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-group-invite-g-1"));
    await waitFor(() => expect(screen.getByTestId("group-invite-picker")).toBeTruthy());
    fireEvent.click(await screen.findByRole("option", { name: /小圆/ }));
    fireEvent.click(screen.getByTestId("group-invite-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("group-invite-error").textContent).toContain("成员数超上限"),
    );
    expect(logSpy).toHaveBeenCalled();
    logSpy.mockRestore();
  });

  it("F09：添加群聊默认直呈建群表单，收到的入群邀请降为次级页签且如实呈现", async () => {
    const mine = groupOf("g-1", "我的群", PEER, [PEER]);
    const joined = { ...groupOf("g-2", "被拉进的群", PEER_B, [PEER_B, PEER]) };
    mocks.groupList.mockResolvedValue([mine, joined]);
    useGroupStore.setState({ groups: [mine, joined] });
    renderSection();
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contacts-group-add"));
    // Radix Tabs 以 mousedown 激活（mouseDown+click 模拟真实按压）
    const invitesTrigger = screen.getByTestId("contacts-group-add-invites");
    fireEvent.mouseDown(invitesTrigger);
    fireEvent.click(invitesTrigger);
    await waitFor(() => expect(screen.getByTestId("contacts-group-invites-list")).toBeTruthy());
    expect(screen.getByTestId("contacts-group-invited-g-2")).toBeTruthy();
    expect(screen.queryByTestId("contacts-group-invited-g-1")).toBeNull();
  });
});
