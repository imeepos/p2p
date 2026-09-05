import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import { seedMockGroup } from "./mock-group-inject";
import {
  addFriend,
  collectGroupEvents,
  groupOf,
  peerId,
  startNode,
  stopIfRunning,
} from "./mock-group-test-utils";
import type { GroupJson } from "./ipc-types";

// mock 群 roster 面（im-group-design §5）：建群校验/owner 权威/rev 收敛/
// 上限 32/退群置位。事件流与发送语义见 mock-group-send.test.ts。

const bus = collectGroupEvents();

beforeEach(async () => {
  vi.useFakeTimers();
  bus.reset();
  await bus.listen();
});

afterEach(async () => {
  bus.release();
  await stopIfRunning();
  vi.useRealTimers();
});

describe("mock group：建群与列表", () => {
  it("groupList 未建群时返回空数组", async () => {
    await expect(mockBackend.groupList()).resolves.toEqual([]);
  });

  it("groupCreate 校验群名/名单/好友簿，成功后 rev=0 并发 chat_group_state", async () => {
    const self = await startNode();
    await expect(mockBackend.groupCreate("   ", [])).rejects.toThrow(/群名/);
    await expect(
      mockBackend.groupCreate("x".repeat(65), [peerId("g1a")]),
    ).rejects.toThrow(/群名/);
    await expect(mockBackend.groupCreate("项目组", [])).rejects.toThrow(/至少需要一名/);
    await expect(
      mockBackend.groupCreate("项目组", [peerId("not-friend")]),
    ).rejects.toThrow(/好友簿/);
    await expect(mockBackend.groupCreate("项目组", [self])).rejects.toThrow(/本机/);

    const a = await addFriend("g1a");
    const group = await mockBackend.groupCreate("  项目组  ", [a, a]);
    expect(group).toMatchObject({
      name: "项目组",
      owner: self,
      rev: 0,
      state: "active",
    });
    expect(group.members).toEqual([self, a]); // 名单去重 + owner 在列
    const rosterEvents = bus.eventsOf("chat_group_state");
    expect(rosterEvents).toHaveLength(1);
    expect((rosterEvents[0] as { group: GroupJson }).group.groupId).toBe(
      group.groupId,
    );
  });

  it("groupList 全量含 left/kicked/disbanded", async () => {
    const self = await startNode();
    const a = await addFriend("g2a");
    await mockBackend.groupCreate("活跃群", [a]);
    seedMockGroup({
      groupId: groupOf("left0001"),
      name: "已退群",
      owner: peerId("g2a"),
      members: [peerId("g2a"), self],
      rev: 3,
      state: "left",
      tsMs: 1,
    });
    const states = (await mockBackend.groupList()).map((g) => g.state);
    expect(states).toContain("active");
    expect(states).toContain("left");
  });
});

describe("mock group：roster 操作面", () => {
  it("owner-only：非 owner 发起 invite/kick/rename 一律 Err，成员可退群", async () => {
    const self = await startNode();
    const foreign = seedMockGroup({
      groupId: groupOf("foreign01"),
      name: "别人的群",
      owner: peerId("owner999"),
      members: [peerId("owner999"), self],
      rev: 5,
      state: "active",
      tsMs: 1,
    });
    await expect(
      mockBackend.groupInvite(foreign.groupId, [peerId("x1")]),
    ).rejects.toThrow(/仅群主/);
    await expect(
      mockBackend.groupKick(foreign.groupId, peerId("owner999")),
    ).rejects.toThrow(/仅群主/);
    await expect(
      mockBackend.groupRename(foreign.groupId, "新名"),
    ).rejects.toThrow(/仅群主/);
    await expect(
      mockBackend.groupDisband(foreign.groupId),
    ).rejects.toThrow(/仅群主/);
    const before = bus.eventsOf("chat_group_state").length;
    const left = await mockBackend.groupLeave(foreign.groupId);
    expect(left.state).toBe("left");
    expect(bus.eventsOf("chat_group_state")).toHaveLength(before + 1);
  });

  it("群主不能退群；invite/kick/rename 各自 rev+1 并发 roster 回执", async () => {
    await startNode();
    const a = await addFriend("g3a");
    const b = await addFriend("g3b");
    const group = await mockBackend.groupCreate("研发群", [a]);

    await expect(mockBackend.groupLeave(group.groupId)).rejects.toThrow(/群主不能退群/);
    await expect(
      mockBackend.groupInvite(group.groupId, [peerId("stranger")]),
    ).rejects.toThrow(/好友簿/);

    const invited = await mockBackend.groupInvite(group.groupId, [b]);
    expect(invited.rev).toBe(1);
    expect(invited.members).toContain(b);
    await expect(
      mockBackend.groupInvite(group.groupId, [b]),
    ).rejects.toThrow(/已在群中/);

    await expect(
      mockBackend.groupKick(group.groupId, invited.owner),
    ).rejects.toThrow(/不能移除群主/);
    await expect(
      mockBackend.groupKick(group.groupId, peerId("nobody")),
    ).rejects.toThrow(/不在群中/);
    const kicked = await mockBackend.groupKick(group.groupId, b);
    expect(kicked.rev).toBe(2);
    expect(kicked.members).not.toContain(b);

    await expect(mockBackend.groupRename(group.groupId, " ")).rejects.toThrow(/群名/);
    const renamed = await mockBackend.groupRename(group.groupId, "  新研发群  ");
    expect(renamed.rev).toBe(3);
    expect(renamed.name).toBe("新研发群");
    expect(bus.eventsOf("chat_group_state")).toHaveLength(4); // 建群+invite+kick+rename
  });

  it("groupDisband：owner 解散 state=disbanded 且 rev+1；非 active 重复解散 Err", async () => {
    await startNode();
    const a = await addFriend("g4a");
    const group = await mockBackend.groupCreate("解散面", [a]);
    const before = bus.eventsOf("chat_group_state").length;

    const disbanded = await mockBackend.groupDisband(group.groupId);
    expect(disbanded.state).toBe("disbanded");
    expect(disbanded.rev).toBe(1);
    const rosterEvents = bus.eventsOf("chat_group_state");
    expect(rosterEvents).toHaveLength(before + 1);
    expect(
      (rosterEvents[rosterEvents.length - 1] as { group: GroupJson }).group.state,
    ).toBe("disbanded");

    // 仅 active 可解散：重复解散显式 Err（幂等保护，不再发回执）
    await expect(mockBackend.groupDisband(group.groupId)).rejects.toThrow(/不可用/);
    expect(bus.eventsOf("chat_group_state")).toHaveLength(before + 1);
  });

  it("群成员上限 32：满员后邀请拒绝", async () => {
    await startNode();
    const self = (await mockBackend.nodeStatus()).peerId!;
    const filler = Array.from({ length: 31 }, (_, i) => peerId(`fill${i}`));
    const group = seedMockGroup({
      groupId: groupOf("cap00000001"),
      name: "大群",
      owner: self,
      members: [self, ...filler],
      rev: 0,
      state: "active",
      tsMs: 1,
    });
    const friend = await addFriend("capfriend");
    await expect(
      mockBackend.groupInvite(group.groupId, [friend]),
    ).rejects.toThrow(/上限/);
  });
});
