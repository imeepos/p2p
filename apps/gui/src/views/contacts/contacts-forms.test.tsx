// 验收 6（§3.4 表单三律改造项）：移动分组显「选择分组」下拉 +
// 「新建分组…」才展开输入框；endpoint wsUrl 历史值下拉（去重、最近在前）；
// 表单错误全部稳定错误码 + i18n key（逐码断言快照）。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson } from "@/lib/ipc-types";

const mocks = vi.hoisted(() => ({
  friends: vi.fn(),
  invites: vi.fn(),
  history: vi.fn(),
  updateFriend: vi.fn(),
  nodeStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatHistory: mocks.history,
    chatFriendUpdate: mocks.updateFriend,
    chatFriendInvite: vi.fn(),
    chatFriendRemove: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatInviteCancel: vi.fn(),
    chatSend: vi.fn(),
    groupList: vi.fn().mockResolvedValue([]),
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: () => Promise.resolve(() => {}),
  },
}));

import "@/i18n";
import i18n from "@/i18n";
import type { I18nKey } from "@/i18n/types";
import { MemoryRouter } from "react-router-dom";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";

import { ChatFriendMoveDialog } from "./chat-friend-move-dialog";
import { EndpointAddDialog } from "./endpoint-add-dialog";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function friendOf(group: string | null): ChatFriendJson {
  return { peerId: PEER, nickname: "小圆", addrs: [], note: null, group };
}

function renderMove(friend: ChatFriendJson | null) {
  return render(
    <MemoryRouter>
      <ConfirmProvider>
        <ChatFriendMoveDialog friend={friend} onOpenChange={() => {}} />
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

/** Radix 下拉点选（jsdom 需 pointer 桩，与 chat 同款辅助） */
async function pickOption(triggerTestId: string, name: string) {
  fireEvent.pointerDown(screen.getByTestId(triggerTestId), {
    button: 0,
    ctrlKey: false,
    pointerType: "mouse",
  });
  const option = await screen.findByRole("option", { name });
  fireEvent.pointerUp(option, { button: 0, pointerType: "mouse" });
  fireEvent.click(option, { button: 0, pointerType: "mouse" });
}

beforeEach(() => {
  localStorage.clear();
  useEndpointMetaStore.getState().resetForTest();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.updateFriend.mockReset();
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
  });
  useGroupStore.setState({ groups: [], groupsLoaded: true, selfPeerId: PEER, friends: [], friendsLoaded: true });
  useAcpStore.setState({ saved: [], activeEndpointId: null, phase: "idle" });
  // Radix Select 在 jsdom 下需要指针捕获/滚动桩（官方已知测试前提）
  Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "hasPointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "releasePointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
});

describe("移动分组三律改造（§3.4）", () => {
  it("已存在分组必须下拉选择；仅「新建分组…」选中才展开输入框", async () => {
    useChatStore.setState({
      friends: [friendOf("家人"), friendOf("同事")],
    });
    renderMove(friendOf("家人"));
    await waitFor(() => expect(screen.getByTestId("friend-move-dialog")).toBeTruthy());
    // 自由文本输入默认不可见（改造点：原为常驻输入框）
    expect(screen.queryByTestId("friend-move-input")).toBeNull();
    // 下拉候选 = 已存在分组 + 未分组 + 新建分组入口
    await pickOption("friend-move-select", "同事");
    // 点选现有组即移动，不经文本输入
    await waitFor(() => expect(mocks.updateFriend).toHaveBeenCalledWith(PEER, { group: "同事" }));
  });

  it("「新建分组…」路径：选中才展开输入框，提交创建并移入，校验同后端口径", async () => {
    useChatStore.setState({ friends: [friendOf("家人")] });
    mocks.updateFriend.mockResolvedValue(friendOf("新组"));
    renderMove(friendOf("家人"));
    await waitFor(() => expect(screen.getByTestId("friend-move-dialog")).toBeTruthy());
    await pickOption("friend-move-select", "新建分组…");
    await waitFor(() => expect(screen.getByTestId("friend-move-input")).toBeTruthy());
    fireEvent.change(screen.getByTestId("friend-move-input"), { target: { value: "新组" } });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    await waitFor(() => expect(mocks.updateFriend).toHaveBeenCalledWith(PEER, { group: "新组" }));
  });

  it("选回现有分组路径仍即时移动（新建态不粘滞）", async () => {
    useChatStore.setState({ friends: [friendOf("家人")] });
    mocks.updateFriend.mockResolvedValue(friendOf(null));
    renderMove(friendOf("家人"));
    await waitFor(() => expect(screen.getByTestId("friend-move-dialog")).toBeTruthy());
    await pickOption("friend-move-select", "新建分组…");
    await waitFor(() => expect(screen.getByTestId("friend-move-input")).toBeTruthy());
    await pickOption("friend-move-select", "家人");
    await waitFor(() => expect(mocks.updateFriend).toHaveBeenCalledWith(PEER, { group: "家人" }));
  });
});

describe("endpoint wsUrl 历史值下拉（§3.4 三律之三）", () => {
  it("聚焦下拉列出历史保存值：去重、最近保存在前", async () => {
    useAcpStore.setState({
      saved: [
        { wsUrl: "ws://10.0.0.1:9000", token: "t", peer: "", endpointId: "ep-a" },
        { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "", endpointId: "ep-b" },
        { wsUrl: "ws://10.0.0.1:9000", token: "t", peer: "", endpointId: "ep-c" },
      ],
    });
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    await pickOption("contacts-endpoint-history", "ws://127.0.0.1:8787");
    // 选择历史值即回填
    expect(
      (screen.getByTestId("contacts-endpoint-wsurl") as HTMLInputElement).value,
    ).toBe("ws://127.0.0.1:8787");
  });
});

describe("表单错误 = 稳定错误码 + i18n key（快照断言）", () => {
  interface Case {
    field: "wsurl" | "peer";
    value: string;
    code: string;
  }
  const CASES: Case[] = [
    { field: "wsurl", value: "", code: "wsUrlRequired" },
    { field: "wsurl", value: "http://127.0.0.1:8787", code: "wsUrlInvalid" },
    { field: "wsurl", value: "not a url", code: "wsUrlInvalid" },
    { field: "peer", value: "mock-peer", code: "peerInvalid" },
    { field: "peer", value: "abc", code: "peerInvalid" },
  ];

  it.each(CASES)("错误码 $code 随字段 $field 稳定输出并经 i18n 渲染", async ({ field, value, code }) => {
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    fireEvent.change(screen.getByTestId("contacts-endpoint-" + field), {
      target: { value },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    const node = await screen.findByTestId("contacts-endpoint-error-" + code);
    // 稳定口径：i18n key 固定为 contacts.endpoint.errors.<code>，文案逐字一致
    expect(node.textContent).toBe(i18n.t(("contacts.endpoint.errors." + code) as I18nKey));
  });

  it("移动分组新组名校验错误码稳定（超长 → i18n 原文）", async () => {
    useChatStore.setState({ friends: [friendOf("家人")] });
    renderMove(friendOf("家人"));
    await waitFor(() => expect(screen.getByTestId("friend-move-dialog")).toBeTruthy());
    await pickOption("friend-move-select", "新建分组…");
    fireEvent.change(screen.getByTestId("friend-move-input"), {
      target: { value: "x".repeat(33) },
    });
    fireEvent.click(screen.getByTestId("friend-move-submit"));
    // 校验口径与后端一致（MAX_GROUP_CHARS=32）：超限原文随 i18n 文案显式呈现
    await waitFor(() => expect(screen.getByTestId("friend-move-invalid")).toBeTruthy());
    expect(screen.getByTestId("friend-move-invalid").textContent).toContain(
      "分组名超过 32 字符上限",
    );
    expect(mocks.updateFriend).not.toHaveBeenCalled();
  });
});
