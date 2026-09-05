import { beforeEach, describe, expect, it, vi } from "vitest";

import type { GroupMessageJson, NodeEventHandler } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";

// §2.3 未读计数（群）：未选中收他人消息 +1、选中清零、自己消息不计。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    handler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: vi.fn().mockResolvedValue([]),
    groupHistory: vi.fn().mockResolvedValue([]),
    nodeStatus: vi.fn().mockResolvedValue({ peerId: "self-peer", running: true }),
    chatFriendsList: vi.fn().mockResolvedValue([]),
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

const GROUP = "g-unread-1";

function groupMessage(overrides: Partial<GroupMessageJson> = {}): GroupMessageJson {
  return {
    id: "gm-" + Math.random().toString(36).slice(2),
    groupId: GROUP,
    senderId: "member-a",
    kind: "text",
    tsMs: Date.now(),
    text: "群消息",
    media: null,
    status: "delivered",
    acks: [],
    ...overrides,
  };
}

function emit(event: Parameters<NodeEventHandler>[0]): void {
  mocks.handler.current?.(event);
}

beforeEach(async () => {
  vi.clearAllMocks();
  useGroupStore.setState({
    groups: [],
    groupsLoaded: true,
    groupsError: null,
    friends: [],
    friendsLoaded: true,
    selfPeerId: "self-peer",
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: {},
    unreadByGroup: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
  await useGroupStore.getState().subscribeEvents();
});

describe("group-store 未读（§2.3）", () => {
  it("未选中群收到他人消息 unread+1；自己消息不计", () => {
    emit({ type: "chat_group_message", groupId: GROUP, message: groupMessage() });
    expect(useGroupStore.getState().unreadByGroup[GROUP]).toBe(1);
    emit({
      type: "chat_group_message",
      groupId: GROUP,
      message: groupMessage({ senderId: "self-peer" }),
    });
    expect(useGroupStore.getState().unreadByGroup[GROUP]).toBe(1);
  });

  it("selectGroup 清零该群，他群不动", async () => {
    emit({ type: "chat_group_message", groupId: GROUP, message: groupMessage() });
    emit({ type: "chat_group_message", groupId: "g-other", message: groupMessage({ groupId: "g-other", id: "gm-o" }) });
    await useGroupStore.getState().selectGroup(GROUP);
    expect(useGroupStore.getState().unreadByGroup[GROUP]).toBe(0);
    expect(useGroupStore.getState().unreadByGroup["g-other"]).toBe(1);
  });

  it("选中期间到达的消息不累积未读", async () => {
    await useGroupStore.getState().selectGroup(GROUP);
    emit({ type: "chat_group_message", groupId: GROUP, message: groupMessage() });
    expect(useGroupStore.getState().unreadByGroup[GROUP] ?? 0).toBe(0);
  });
});
