import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GroupInviteJson } from "@/lib/ipc-types";

import "@/i18n";

// IMC3 群邀请切片：初次加载 / chat_group_invite 增量 upsert / 状态迁移幂等 /
// 徽标计数 selector / 同意拒绝动作的乐观收敛（IPC mock 命令名与契约逐字一致）。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    chatGroupInvitesList: vi.fn(),
    chatGroupInviteSend: vi.fn(),
    chatGroupInviteAccept: vi.fn(),
    chatGroupInviteReject: vi.fn(),
    handlers: [] as ((e: unknown) => void)[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatGroupInvitesList: mocks.chatGroupInvitesList,
    chatGroupInviteSend: mocks.chatGroupInviteSend,
    chatGroupInviteAccept: mocks.chatGroupInviteAccept,
    chatGroupInviteReject: mocks.chatGroupInviteReject,
    onNodeEvent: (handler: (e: unknown) => void) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

import { useChatStore } from "./chat-store";
import {
  selectPendingInviteBadgeCount,
  upsertGroupInviteInto,
} from "./chat-group-invite-slice";

export function inviteOf(patch: Partial<GroupInviteJson>): GroupInviteJson {
  return {
    id: "inv-1",
    groupId: "g-1",
    groupName: "项目组",
    owner: "owner-peer",
    inviter: "inviter-peer",
    invitee: "invitee-peer",
    note: null,
    direction: "in",
    state: "pending",
    tsMs: 1000,
    delivered: true,
    ...patch,
  };
}

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as ReturnType<typeof vi.fn>).mockReset();
  }
  useChatStore.setState({
    groupInvites: [],
    groupInvitesLoaded: false,
    groupInvitesError: null,
    invites: [],
  });
});

describe("upsertGroupInviteInto（增量与幂等）", () => {
  const base = inviteOf({});

  it("未见过的 id 追加", () => {
    const next = upsertGroupInviteInto([], base);
    expect(next).toEqual([base]);
  });

  it("pending 条目被来料覆盖（状态推进）", () => {
    const next = upsertGroupInviteInto([base], inviteOf({ id: "inv-1", state: "accepted" }));
    expect(next![0]!.state).toBe("accepted");
  });

  it("同 id 同内容 pending 重复事件幂等（返回 null 不触发更新）", () => {
    expect(upsertGroupInviteInto([base], { ...base })).toBeNull();
  });

  it("终态条目不回退：来料 pending 视为迟到事件忽略", () => {
    const accepted = inviteOf({ state: "accepted" });
    expect(upsertGroupInviteInto([accepted], { ...base, state: "pending" })).toBeNull();
  });

  it("终态条目忽略另一终态来料（首个终态为准）", () => {
    const accepted = inviteOf({ state: "accepted" });
    expect(upsertGroupInviteInto([accepted], { ...base, state: "rejected" })).toBeNull();
  });
});

describe("loadGroupInvites（初次 list 加载）", () => {
  it("成功装载并清除错误", async () => {
    mocks.chatGroupInvitesList.mockResolvedValue([inviteOf({})]);
    await useChatStore.getState().loadGroupInvites();
    const s = useChatStore.getState();
    expect(s.groupInvites).toHaveLength(1);
    expect(s.groupInvitesLoaded).toBe(true);
    expect(s.groupInvitesError).toBeNull();
  });

  it("失败原文落 groupInvitesError 不静默", async () => {
    mocks.chatGroupInvitesList.mockRejectedValue(new Error("list failed"));
    await useChatStore.getState().loadGroupInvites();
    const s = useChatStore.getState();
    expect(s.groupInvitesLoaded).toBe(true);
    expect(s.groupInvitesError).toBe("list failed");
  });
});

describe("动作与事件接线", () => {
  it("chat_group_invite 事件经 subscribeEvents 增量 upsert，重复投递幂等", async () => {
    await useChatStore.getState().subscribeEvents();
    const event = {
      type: "chat_group_invite" as const,
      invite: inviteOf({}),
      tsMs: 1,
    };
    for (const handler of mocks.handlers) handler(event);
    for (const handler of mocks.handlers) handler(event);
    expect(useChatStore.getState().groupInvites).toHaveLength(1);
  });

  it("acceptGroupInvite 成功后乐观收敛为 accepted", async () => {
    useChatStore.setState({ groupInvites: [inviteOf({})] });
    mocks.chatGroupInviteAccept.mockResolvedValue(undefined);
    await useChatStore.getState().acceptGroupInvite("inv-1");
    expect(mocks.chatGroupInviteAccept).toHaveBeenCalledWith("inv-1");
    expect(useChatStore.getState().groupInvites[0]!.state).toBe("accepted");
  });

  it("acceptGroupInvite 失败原样上抛（状态不动）", async () => {
    useChatStore.setState({ groupInvites: [inviteOf({})] });
    mocks.chatGroupInviteAccept.mockRejectedValue(new Error("boom"));
    await expect(useChatStore.getState().acceptGroupInvite("inv-1")).rejects.toThrow("boom");
    expect(useChatStore.getState().groupInvites[0]!.state).toBe("pending");
  });

  it("rejectGroupInvite 成功后乐观收敛为 rejected", async () => {
    useChatStore.setState({ groupInvites: [inviteOf({})] });
    mocks.chatGroupInviteReject.mockResolvedValue(undefined);
    await useChatStore.getState().rejectGroupInvite("inv-1", "太吵了");
    expect(mocks.chatGroupInviteReject).toHaveBeenCalledWith("inv-1", "太吵了");
    expect(useChatStore.getState().groupInvites[0]!.state).toBe("rejected");
  });

  it("sendGroupInvite 返回条目并入库", async () => {
    const sent = inviteOf({ id: "inv-9", direction: "out", inviter: "me", invitee: "p9" });
    mocks.chatGroupInviteSend.mockResolvedValue(sent);
    const result = await useChatStore.getState().sendGroupInvite("g-1", "p9", null);
    expect(mocks.chatGroupInviteSend).toHaveBeenCalledWith("g-1", "p9", null);
    expect(result.id).toBe("inv-9");
    expect(useChatStore.getState().groupInvites.map((i) => i.id)).toContain("inv-9");
  });
});

describe("selectPendingInviteBadgeCount（铃铛徽标）", () => {
  it("入群邀请 in 向 pending + 好友邀请 in 向求和", () => {
    const s = {
      groupInvites: [
        inviteOf({ id: "a" }),
        inviteOf({ id: "b", state: "accepted" as const }),
        inviteOf({ id: "c", direction: "out" as const }),
      ],
      invites: [
        { direction: "in" },
        { direction: "in" },
        { direction: "out" },
      ],
    };
    expect(selectPendingInviteBadgeCount(s)).toBe(3);
  });

  it("无待处理为 0", () => {
    expect(selectPendingInviteBadgeCount({ groupInvites: [], invites: [] })).toBe(0);
  });
});
