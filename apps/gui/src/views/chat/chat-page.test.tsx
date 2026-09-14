import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  GroupJson,
  NodeEventHandler,
} from "@/lib/ipc-types";
import { useAcpStore } from "@/acp/acp-store";
import { resetWorkspaceUiForTest } from "@/acp/workspace-ui-store";
import { useGroupStore } from "@/stores/group-store";
import { useUiPrefsStore } from "@/stores/ui-prefs-store";
import {
  ENDPOINT_ID,
  GROUP_ID,
  PEER,
  PEER_B,
  groupFixture,
  seedAll,
  renderAt,
  type MediaMocks,
} from "@/test/chat-page-fixtures";
import type { Mock } from "vitest";

import "@/i18n";

// P1 聚合聊天验收（§2.1/§2.2/§2.3）：三来源混排、路由化深链选中、
// 无 query 空态、kind 聚焦（拍板项 1）。窄屏互斥与搜索见 chat-page-layout。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<(peer: string) => Promise<ChatMessageJson[]>>(),
    send: vi.fn(),
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    invites: vi.fn<() => Promise<never[]>>(),
    nodeStatus: vi.fn(),
    handlers: [] as NodeEventHandler[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    chatInvitesList: mocks.invites,
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

function seed(): void {
  seedAll(mocks as unknown as MediaMocks);
}

function emit(event: Parameters<NodeEventHandler>[0]): void {
  act(() => {
    for (const handler of mocks.handlers) handler(event);
  });
}

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as Mock).mockReset();
  }
  // 不清 handlers：store 订阅为模块级单例，重复注册的归并幂等（按消息 id 去重）
  seed();
  // T5：agent 段树的开合态为模块级 zustand，逐用例复位防跨用例串态
  resetWorkspaceUiForTest();
});

describe("三来源分段渲染（§2.2 + T5 树移植）", () => {
  it("agent 段两级树在前，好友/群扁平段按 lastTsMs 降序混排", async () => {
    renderAt();
    const friendRow = await screen.findByTestId("conversation-row-friend-" + PEER);
    expect(friendRow.textContent).toContain("小圆");
    expect(friendRow.textContent).toContain("备注甲");
    const groupRow = screen.getByTestId("conversation-row-group-" + GROUP_ID);
    expect(groupRow.textContent).toContain("项目组");
    expect(groupRow.textContent).toContain("阿北：开会啦");
    const agentRow = screen.getByTestId("conversation-row-agent-" + ENDPOINT_ID);
    expect(agentRow.textContent).toContain("助手甲");
    expect(agentRow.textContent).toContain("未连接");
    // T5 分段：agent 条目进两级树（host 组头 + agent 行），好友/群保持扁平；
    // 扁平段相对序不变：群(4000) > friend PEER(3000) > friend PEER_B(2000)
    const list = screen.getByTestId("conversation-items");
    const order = [...list.querySelectorAll("button")].map((b) =>
      b.getAttribute("data-testid"),
    );
    expect(order).toEqual([
      "acp-group-row-127.0.0.1:8787",
      "conversation-row-agent-" + ENDPOINT_ID,
      "conversation-row-group-" + GROUP_ID,
      "conversation-row-friend-" + PEER,
      "conversation-row-friend-" + PEER_B,
    ]);
  });

  it("未选中会话收消息：条目未读 +1 呈现（store 语义另有专测）", async () => {
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    // 订阅为异步挂载面：等事件回调注册完成再注入
    await waitFor(() => expect(mocks.handlers.length).toBeGreaterThan(0));
    emit({
      type: "chat_message",
      peer: PEER,
      message: { id: "pf9", peer: PEER, sender: "them", kind: "text", tsMs: Date.now(), text: "新消息", media: null, status: "delivered" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("conversation-unread-" + PEER).textContent).toBe("1"),
    );
  });
});

describe("路由化选中态（§2.1 深链）", () => {
  it("?peer= 直落 1:1 会话：历史装载、输入条就位、头部显昵称", async () => {
    renderAt("/chat?peer=" + PEER);
    await waitFor(() => expect(mocks.history).toHaveBeenCalledWith(PEER, null, 20));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
    expect(screen.getByTestId("chat-conversation-header").textContent).toContain("小圆");
  });

  it("?group= 直落群会话：群头部与输入条就位", async () => {
    renderAt("/chat?group=" + GROUP_ID);
    await waitFor(() => expect(screen.getByTestId("group-conversation-header")).toBeTruthy());
    expect(screen.getByTestId("group-conversation-header").textContent).toContain("项目组");
    expect(screen.getByTestId("group-input")).toBeTruthy();
  });

  it("?agent= 直落 agent 会话：未连接显连接引导卡，聚焦落定", async () => {
    useAcpStore.setState({ unreadByEndpoint: { [ENDPOINT_ID]: 2 } });
    renderAt("/chat?agent=" + ENDPOINT_ID);
    await waitFor(() => expect(screen.getByTestId("agent-connect-card")).toBeTruthy());
    expect(screen.getByTestId("agent-connect")).toBeTruthy();
    // §2.3 聚焦落定即清零该端点未读
    expect(useAcpStore.getState().focusedEndpointId).toBe(ENDPOINT_ID);
    await waitFor(() =>
      expect(useAcpStore.getState().unreadByEndpoint[ENDPOINT_ID] ?? 0).toBe(0),
    );
  });

  it("无 query：右侧空态「选择或发起会话」，列表仍可见", async () => {
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    expect(screen.getByText("选择或发起会话")).toBeTruthy();
    expect(screen.queryByTestId("chat-input")).toBeNull();
  });

  it("?kind=group 聚焦排序最前的群条目（拍板项 1）", async () => {
    renderAt("/chat?kind=group");
    await waitFor(() => expect(screen.getByTestId("group-conversation-header")).toBeTruthy());
  });

  it("?kind=group 且无群条目：保持空态不误选", async () => {
    mocks.groupList.mockResolvedValue([]);
    useGroupStore.setState({ groups: [], lastMessageByGroup: {} });
    renderAt("/chat?kind=group");
    await screen.findByTestId("conversation-row-friend-" + PEER);
    expect(screen.getByText("选择或发起会话")).toBeTruthy();
  });
});

describe("已退出/已解散群聊默认隐藏（IM 开关可显）", () => {
  const LEFT_ID = "99999999-2222-3333-4444-555555555555";

  beforeEach(() => {
    localStorage.clear();
    useUiPrefsStore.setState({ showInactiveGroups: false });
  });

  function seedLeftGroup(): void {
    const left = { ...groupFixture(), groupId: LEFT_ID, name: "旧群", state: "left" as const, tsMs: 400 };
    mocks.groupList.mockResolvedValue([groupFixture(), left]);
    useGroupStore.setState({
      groups: [groupFixture(), left],
      lastMessageByGroup: { ...useGroupStore.getState().lastMessageByGroup, [LEFT_ID]: null },
    });
  }

  it("默认不渲染非 active 群条目，侧栏底部出开关", async () => {
    seedLeftGroup();
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    expect(screen.queryByTestId("conversation-row-group-" + LEFT_ID)).toBeNull();
    expect(screen.getByTestId("conversation-row-group-" + GROUP_ID)).toBeTruthy();
    expect(screen.getByTestId("inactive-groups-toggle")).toBeTruthy();
  });

  it("打开开关后非 active 群条目恢复显示", async () => {
    seedLeftGroup();
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    fireEvent.click(screen.getByTestId("inactive-groups-switch"));
    expect(await screen.findByTestId("conversation-row-group-" + LEFT_ID)).toBeTruthy();
  });
});

describe("T5 侧栏移植渲染矩阵", () => {
  it("agent 段两级树：分组头渲染，折叠隐藏 agent 行、群/好友行不受影响", async () => {
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    const head = screen.getByTestId("acp-group-row-127.0.0.1:8787");
    expect(head.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(head);
    expect(screen.queryByTestId("conversation-row-agent-" + ENDPOINT_ID)).toBeNull();
    expect(screen.getByTestId("conversation-row-group-" + GROUP_ID)).toBeTruthy();
    expect(screen.getByTestId("conversation-row-friend-" + PEER)).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-group-row-127.0.0.1:8787"));
    expect(screen.getByTestId("conversation-row-agent-" + ENDPOINT_ID)).toBeTruthy();
  });

  it("?agent= 深链选中：所在组 folder 高亮 info 且强制展开", async () => {
    renderAt("/chat?agent=" + ENDPOINT_ID);
    await screen.findByTestId("conversation-row-agent-" + ENDPOINT_ID);
    const folder = screen
      .getByTestId("acp-group-row-127.0.0.1:8787")
      .querySelector("svg");
    expect(folder?.className.baseVal).toContain("text-info");
    // 含当前会话的组即使点了组头也不收敛（isGroupOpen 强制展开）
    fireEvent.click(screen.getByTestId("acp-group-row-127.0.0.1:8787"));
    expect(screen.getByTestId("conversation-row-agent-" + ENDPOINT_ID)).toBeTruthy();
  });

  it("好友/群段条目与移植前等价：标题/备注/预览逐项断言", async () => {
    renderAt();
    const friendRow = await screen.findByTestId("conversation-row-friend-" + PEER);
    expect(friendRow.textContent).toContain("小圆");
    expect(friendRow.textContent).toContain("备注甲");
    expect(friendRow.textContent).toContain("晚安");
    const friendBRow = screen.getByTestId("conversation-row-friend-" + PEER_B);
    expect(friendBRow.textContent).toContain("阿北");
    expect(friendBRow.textContent).toContain("早");
    const groupRow = screen.getByTestId("conversation-row-group-" + GROUP_ID);
    expect(groupRow.textContent).toContain("项目组");
    expect(groupRow.textContent).toContain("阿北：开会啦");
  });
});