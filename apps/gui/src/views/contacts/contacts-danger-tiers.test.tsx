// 验收 7（§3.5 破坏性动作确认三档制，全应用规范本期落验收）：
// 各危险动作落入正确档位的抽检断言；重置身份输入确认文本（第一档）；
// 第二档红钮 + 后果明示文案；第三档单次确认。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, GroupJson } from "@/lib/ipc-types";

const mocks = vi.hoisted(() => ({
  friends: vi.fn(),
  invites: vi.fn(),
  history: vi.fn(),
  removeFriend: vi.fn(),
  groupList: vi.fn(),
  groupLeave: vi.fn(),
  identityReset: vi.fn(),
  nodeStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatHistory: mocks.history,
    chatFriendRemove: mocks.removeFriend,
    chatFriendInvite: vi.fn(),
    chatFriendUpdate: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatInviteCancel: vi.fn(),
    chatSend: vi.fn(),
    groupList: mocks.groupList,
    groupLeave: mocks.groupLeave,
    groupCreate: vi.fn(),
    groupInvite: vi.fn(),
    groupKick: vi.fn(),
    groupRename: vi.fn(),
    groupDisband: vi.fn(),
    groupHistory: vi.fn().mockResolvedValue([]),
    groupSend: vi.fn(),
    identityReset: mocks.identityReset,
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: () => Promise.resolve(() => {}),
  },
}));
vi.mock("@/stores/node-store", async (importOriginal) => {
  const mod = await importOriginal<typeof import("@/stores/node-store")>();
  return {
    ...mod,
    useNodeStore: {
      ...mod.useNodeStore,
      getState: () => ({ ...mod.useNodeStore.getState(), refresh: vi.fn().mockResolvedValue(undefined) }),
    },
  };
});

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { ChatFriendRemoveDialog } from "./chat-friend-remove-dialog";
import { AgentSection } from "./agent-section";
import { GroupSection } from "./group-section";
import { ResetIdentityDialog } from "@/views/settings/reset-identity-dialog";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "2jSUsWcEf7z68xBscf2YmVYzQ4uPZfpMz8XRW3vruJU4";

function friendOf(): ChatFriendJson {
  return { peerId: PEER, nickname: "小圆", addrs: [], note: null, group: null };
}

function groupOf(): GroupJson {
  return {
    groupId: "g-1",
    name: "旧群",
    owner: PEER_B,
    members: [PEER_B, PEER],
    rev: 1,
    state: "active",
    tsMs: 1_000,
  };
}

function renderTree(node: React.ReactElement) {
  return render(
    <MemoryRouter>
      <ConfirmProvider>{node}</ConfirmProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  localStorage.clear();
  useEndpointMetaStore.getState().resetForTest();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.removeFriend.mockReset().mockResolvedValue(true);
  mocks.groupList.mockReset().mockResolvedValue([]);
  mocks.groupLeave.mockReset().mockResolvedValue(groupOf());
  mocks.identityReset.mockReset().mockResolvedValue({
    running: false,
    peerId: null,
    listenAddrs: [],
    uptimeSecs: 0,
    startedAtMs: null,
    config: {},
  });
  mocks.nodeStatus.mockReset().mockResolvedValue({
    running: true,
    peerId: PEER,
    listenAddrs: [],
    uptimeSecs: 1,
    startedAtMs: 1,
    config: {},
  });
  useChatStore.setState({ invites: [], friends: [], friendsLoaded: true, friendsError: null, selectedPeer: null, messagesByPeer: {}, lastMessageByPeer: {}, unreadByPeer: {} });
  useGroupStore.setState({ groups: [], groupsLoaded: true, selfPeerId: PEER, friends: [], friendsLoaded: true, unreadByGroup: {}, selectedGroupId: null });
  useAcpStore.setState({ saved: [], activeEndpointId: null, phase: "idle" });
  Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
  Object.defineProperty(window.HTMLElement.prototype, "hasPointerCapture", { configurable: true, value: vi.fn() });
  Object.defineProperty(window.HTMLElement.prototype, "releasePointerCapture", { configurable: true, value: vi.fn() });
});

// 三档制归属表（§3.5）：落点即文档；行为断言见下方用例与既有测试。
// | 动作 | 档位 | 落点 |
// | 重置身份 | 一（输入确认文本） | settings/reset-identity-dialog（既有测试覆盖）|
// | 解散群/踢出成员 | 二 | views/group/group-member-panel（destructive confirm）|
// | 删好友 | 二 | contacts/chat-friend-remove-dialog（本文件）|
// | 删除 agent endpoint | 二（含索引移除说明）| contacts（本文件）|
// | 退群 | 三（单次确认）| contacts/group-section（本文件）|
// | 停用 agent | 三（单次确认，可逆）| contacts（本文件）|
// | 清理日志/缓存 | 三 | views/network/diagnostics（P3 已落，destructive 单次确认）|
describe("三档制抽检（P2 验收 7）", () => {
  it("第一档 重置身份：确认钮须输入 PeerId 前缀确认文本方可执行", async () => {
    renderTree(<ResetIdentityDialog peerId={PEER} />);
    fireEvent.click(screen.getByRole("button", { name: "重置身份" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("重置节点身份？");
    // 未输入：不可执行
    const confirm = screen.getByRole("button", { name: "确认重置" }) as HTMLButtonElement;
    expect(confirm.disabled).toBe(true);
    expect(mocks.identityReset).not.toHaveBeenCalled();
    // 错误输入：仍不可执行
    fireEvent.change(screen.getByPlaceholderText("输入当前 PeerId 前 4 位以确认"), {
      target: { value: "XXXX" },
    });
    expect(confirm.disabled).toBe(true);
    // 正确输入确认文本：方可执行
    fireEvent.change(screen.getByPlaceholderText("输入当前 PeerId 前 4 位以确认"), {
      target: { value: PEER.slice(0, 4) },
    });
    expect(confirm.disabled).toBe(false);
  });

  it("第二档 删好友：二次确认 + 红色按钮 + 后果明示文案", async () => {
    useChatStore.setState({ friends: [friendOf()], friendsLoaded: true });
    renderTree(<ChatFriendRemoveDialog friend={friendOf()} onOpenChange={() => {}} />);
    await waitFor(() => expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy());
    const dialog = screen.getByTestId("friend-remove-dialog");
    // 后果明示：说明移除对象 + 本地消息历史保留语义
    expect(dialog.textContent).toContain("小圆");
    expect(dialog.textContent).toContain("不会删除本地消息历史");
    // 红钮：destructive 档确认按钮
    const confirm = screen.getByTestId("friend-remove-confirm");
    expect(confirm.className).toContain("bg-destructive");
    // 默认焦点在取消（防误触），确认恰好调用一次
    fireEvent.click(confirm);
    await waitFor(() => expect(mocks.removeFriend).toHaveBeenCalledTimes(1));
  });

  it("第二档 删除 agent endpoint：红钮 + 「移除本地会话记录索引」影响说明；确认后配置与元数据一并清除", async () => {
    useAcpStore.setState({
      saved: [{ wsUrl: "ws://127.0.0.1:8787", token: "t", peer: PEER_B, alias: "助手", endpointId: "ep-1" }],
    });
    useEndpointMetaStore.getState().setPolicy("ep-1", { defaults: { read: "allow" }, exceptions: [] });
    useEndpointMetaStore.getState().recordTest("ep-1", "ok");
    renderTree(<AgentSection />);
    await waitFor(() => expect(screen.getByTestId("contact-agent-ep-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-agent-remove-ep-1"));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("移除本地会话记录索引");
    const confirm = screen.getByRole("button", { name: "删除" });
    expect((confirm as HTMLElement).className).toContain("bg-destructive");
    fireEvent.click(confirm);
    await waitFor(() => expect(useAcpStore.getState().saved.length).toBe(0));
    // 元数据同步清场，不留孤儿键
    expect(useEndpointMetaStore.getState().policies["ep-1"]).toBeUndefined();
    expect(useEndpointMetaStore.getState().lastTest["ep-1"]).toBeUndefined();
  });

  it("第三档 退群：单次确认即执行（数据仅置位保留语义）", async () => {
    mocks.groupList.mockResolvedValue([groupOf()]);
    useGroupStore.setState({ groups: [groupOf()] });
    renderTree(<GroupSection />);
    await waitFor(() => expect(screen.getByTestId("contact-group-g-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-group-leave-g-1"));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("历史保留");
    fireEvent.click(screen.getByRole("button", { name: "退出" }));
    await waitFor(() => expect(mocks.groupLeave).toHaveBeenCalledWith("g-1"));
    expect(mocks.groupLeave).toHaveBeenCalledTimes(1);
  });

  it("第三档 停用 agent：单次确认即停用（保留配置可逆）", async () => {
    useAcpStore.setState({
      saved: [{ wsUrl: "ws://127.0.0.1:8787", token: "t", peer: PEER_B, alias: "助手", endpointId: "ep-1" }],
    });
    renderTree(<AgentSection />);
    await waitFor(() => expect(screen.getByTestId("contact-agent-ep-1")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contact-agent-disable-ep-1"));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("保留配置");
    fireEvent.click(screen.getByRole("button", { name: "停用" }));
    await waitFor(() =>
      expect(useEndpointMetaStore.getState().disabled["ep-1"]).toBe(true),
    );
    await waitFor(() => expect(screen.getByTestId("contact-agent-disabled-ep-1")).toBeTruthy());
  });
});
