import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  GroupJson,
  GroupMessageJson,
  GroupSendReport,
  NodeEventHandler,
  NodeStatus,
} from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<
      (groupId: string, beforeId?: string | null, limit?: number) => Promise<GroupMessageJson[]>
    >(),
    groupSend: vi.fn<() => Promise<GroupSendReport>>(),
    groupInvite: vi.fn<() => Promise<GroupJson>>(),
    nodeStatus: vi.fn<() => Promise<NodeStatus>>(),
    handler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    groupSend: mocks.groupSend,
    groupInvite: mocks.groupInvite,
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import { useGroupStore } from "./group-store";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

const SELF = peerId("self");
const GROUP = "00000000-0000-4000-8000-000000000001";

function groupOf(id: string, patch: Partial<GroupJson> = {}): GroupJson {
  return {
    groupId: id,
    name: "项目组",
    owner: SELF,
    members: [SELF, peerId("m1")],
    rev: 0,
    state: "active",
    tsMs: 1,
    ...patch,
  };
}

function gmsg(
  id: string,
  senderId: string,
  text: string,
  tsMs: number,
  patch: Partial<GroupMessageJson> = {},
): GroupMessageJson {
  return {
    id,
    groupId: GROUP,
    senderId,
    kind: "text",
    tsMs,
    text,
    media: null,
    status: "delivered",
    acks: [],
    ...patch,
  };
}

function sendReport(message: GroupMessageJson): GroupSendReport {
  return { message, acked: 1, recipients: 1, delivered: true };
}

function emit(event: Parameters<NodeEventHandler>[0]): void {
  mocks.handler.current?.(event);
}

beforeEach(() => {
  vi.clearAllMocks();
  useGroupStore.setState({
    groups: [],
    groupsLoaded: false,
    groupsError: null,
    friends: [],
    friendsLoaded: false,
    selfPeerId: SELF,
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
});

describe("group-store 列表与本机身份", () => {
  it("loadGroups 成功落列表；失败落 groupsError 不静默", async () => {
    mocks.groupList.mockResolvedValueOnce([groupOf(GROUP)]);
    await useGroupStore.getState().loadGroups();
    expect(useGroupStore.getState().groups).toHaveLength(1);
    expect(useGroupStore.getState().groupsLoaded).toBe(true);

    mocks.groupList.mockRejectedValueOnce(new Error("list boom"));
    await useGroupStore.getState().loadGroups();
    expect(useGroupStore.getState().groupsError).toBe("list boom");
  });

  it("refreshSelf 读节点状态 peerId；节点停止时为 null", async () => {
    mocks.nodeStatus.mockResolvedValueOnce({
      running: true,
      peerId: SELF,
    } as NodeStatus);
    await useGroupStore.getState().refreshSelf();
    expect(useGroupStore.getState().selfPeerId).toBe(SELF);

    mocks.nodeStatus.mockResolvedValueOnce({ running: false, peerId: null } as NodeStatus);
    await useGroupStore.getState().refreshSelf();
    expect(useGroupStore.getState().selfPeerId).toBeNull();
  });
});

describe("group-store 历史与发送", () => {
  it("selectGroup 拉一页历史且已加载不重复拉取", async () => {
    mocks.groupHistory.mockResolvedValue([gmsg("g1", SELF, "早", 1)]);
    await useGroupStore.getState().selectGroup(GROUP);
    await useGroupStore.getState().selectGroup(GROUP);
    expect(mocks.groupHistory).toHaveBeenCalledTimes(1);
    expect(mocks.groupHistory).toHaveBeenCalledWith(GROUP, null, 50);
    expect(useGroupStore.getState().messagesByGroup[GROUP].map((m) => m.id)).toEqual(["g1"]);
  });

  it("selectGroup 失败落 historyError，重试成功清除", async () => {
    mocks.groupHistory.mockRejectedValueOnce(new Error("history boom"));
    await expect(useGroupStore.getState().selectGroup(GROUP)).rejects.toThrow("history boom");
    expect(useGroupStore.getState().historyError[GROUP]).toBe("history boom");
    mocks.groupHistory.mockResolvedValueOnce([]);
    await useGroupStore.getState().selectGroup(GROUP);
    expect(useGroupStore.getState().historyError[GROUP]).toBeNull();
  });

  it("sendText 乐观占位换真身；失败回滚占位并抛错", async () => {
    mocks.groupHistory.mockResolvedValue([]);
    await useGroupStore.getState().selectGroup(GROUP);
    const real = gmsg("real-1", SELF, "你好", Date.now(), { acks: [peerId("m1")] });
    mocks.groupSend.mockResolvedValueOnce(sendReport(real));

    await useGroupStore.getState().sendText(GROUP, "你好");
    const list = useGroupStore.getState().messagesByGroup[GROUP];
    expect(list.map((m) => m.id)).toEqual(["real-1"]);
    expect(useGroupStore.getState().lastMessageByGroup[GROUP]?.id).toBe("real-1");

    mocks.groupSend.mockRejectedValueOnce(new Error("send boom"));
    await expect(useGroupStore.getState().sendText(GROUP, "第二条")).rejects.toThrow("send boom");
    expect(useGroupStore.getState().messagesByGroup[GROUP].map((m) => m.id)).toEqual(["real-1"]);
  });

  it("upsertGroup 按 groupId 替换保序，未知群追加", async () => {
    useGroupStore.getState().upsertGroup(groupOf("g-a", { name: "A" }));
    useGroupStore.getState().upsertGroup(groupOf("g-b", { name: "B" }));
    useGroupStore.getState().upsertGroup(groupOf("g-a", { name: "A2", rev: 2 }));
    const groups = useGroupStore.getState().groups;
    expect(groups.map((g) => g.name)).toEqual(["A2", "B"]);
    expect(groups[0]!.rev).toBe(2);
  });

  it("invite 命令返回值回写群列表", async () => {
    const invited = groupOf(GROUP, { members: [SELF, peerId("m1"), peerId("m2")], rev: 1 });
    mocks.groupInvite.mockResolvedValueOnce(invited);
    await useGroupStore.getState().invite(GROUP, [peerId("m2")]);
    expect(useGroupStore.getState().groups).toEqual([invited]);
  });
});

describe("group-store 事件归并", () => {
  it("chat_group_message 追加并按 id 去重；摘要随事件刷新", async () => {
    await useGroupStore.getState().subscribeEvents();
    emit({ type: "chat_group_message", groupId: GROUP, message: gmsg("e1", peerId("m1"), "早到", 2) });
    emit({ type: "chat_group_message", groupId: GROUP, message: gmsg("e1", peerId("m1"), "早到", 2) });
    emit({ type: "chat_group_message", groupId: GROUP, message: gmsg("e2", peerId("m1"), "第二条", 3) });
    const list = useGroupStore.getState().messagesByGroup[GROUP];
    expect(list.map((m) => m.id)).toEqual(["e1", "e2"]);
    expect(useGroupStore.getState().lastMessageByGroup[GROUP]?.id).toBe("e2");
  });

  it("chat_group_status 原地推进 acks 与状态", async () => {
    await useGroupStore.getState().subscribeEvents();
    emit({
      type: "chat_group_message",
      groupId: GROUP,
      message: gmsg("m1", SELF, "在吗", 1, { status: "pending" }),
    });
    emit({
      type: "chat_group_status",
      groupId: GROUP,
      messageId: "m1",
      acks: [peerId("m1")],
      status: "delivered",
    });
    const message = useGroupStore.getState().messagesByGroup[GROUP][0]!;
    expect(message.acks).toEqual([peerId("m1")]);
    expect(message.status).toBe("delivered");
  });

  it("chat_group_state 回执 upsert 群列表（建群/roster 变更可见）", async () => {
    await useGroupStore.getState().subscribeEvents();
    emit({ type: "chat_group_state", group: groupOf(GROUP, { name: "事件群" }) });
    expect(useGroupStore.getState().groups).toEqual([groupOf(GROUP, { name: "事件群" })]);
  });
});
