import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import { injectMockGroupIncoming, seedMockGroup } from "./mock-group-inject";
import type {
  GuiConfig,
  GroupJson,
  GroupMessageJson,
  NodeEventJson,
} from "./ipc-types";

// mock 群聊段（im-group-design §7）：roster 校验/rev 收敛/ack 事件流/
// 历史分页/媒体占位。state 是模块级单例，测试间共享——用例用独立 seed 隔离，
// 「空列表」断言必须位于文件首个 groupList 调用。

const CFG: GuiConfig = {
  quicPort: 34000,
  tcpPort: 34001,
  enableMdns: true,
  dataDir: "/tmp/mock",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

const ADDR = "192.168.1.5/u3400";
const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

function groupOf(seed: string): string {
  return `00000000-0000-4000-8000-${seed.padStart(12, "0").slice(-12)}`;
}

let events: NodeEventJson[] = [];
let unlisten: (() => void) | null = null;

function eventsOf(type: NodeEventJson["type"]): NodeEventJson[] {
  return events.filter((e) => e.type === type);
}

async function startNode(): Promise<string> {
  const start = mockBackend.nodeStart(CFG);
  await vi.advanceTimersByTimeAsync(1000);
  const status = await start;
  return status.peerId!;
}

async function stopIfRunning() {
  const status = await mockBackend.nodeStatus();
  if (status.running) {
    const stop = mockBackend.nodeStop();
    await vi.advanceTimersByTimeAsync(500);
    await stop;
  }
}

async function addFriend(seed: string): Promise<string> {
  const peer = peerId(seed);
  await mockBackend.chatFriendAdd(peer, `好友${seed}`, [ADDR]);
  return peer;
}

async function connect(peer: string): Promise<void> {
  const dial = mockBackend.peerConnect(peer);
  await vi.advanceTimersByTimeAsync(1000);
  await dial;
}

async function createGroupWith(name: string, members: string[]): Promise<GroupJson> {
  return mockBackend.groupCreate(name, members);
}

beforeEach(async () => {
  vi.useFakeTimers();
  events = [];
  unlisten = await mockBackend.onNodeEvent((event) => events.push(event));
});

afterEach(async () => {
  unlisten?.();
  unlisten = null;
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
    const group = await createGroupWith("  项目组  ", [a, a]);
    expect(group).toMatchObject({
      name: "项目组",
      owner: self,
      rev: 0,
      state: "active",
    });
    expect(group.members).toEqual([self, a]); // 名单去重 + owner 在列
    const rosterEvents = eventsOf("chat_group_state");
    expect(rosterEvents).toHaveLength(1);
    expect((rosterEvents[0] as { group: GroupJson }).group.groupId).toBe(
      group.groupId,
    );
  });

  it("groupList 全量含 left/kicked/disbanded", async () => {
    const a = await addFriend("g2a");
    await createGroupWith("活跃群", [a]);
    seedMockGroup({
      groupId: groupOf("left0001"),
      name: "已退群",
      owner: peerId("g2a"),
      members: [peerId("g2a"), (await mockBackend.nodeStatus()).peerId!],
      rev: 3,
      state: "left",
      tsMs: 1,
    });
    const groups = await mockBackend.groupList();
    expect(groups.map((g) => g.state)).toContain("active");
    expect(groups.map((g) => g.state)).toContain("left");
  });
});

describe("mock group：roster 操作面", () => {
  it("owner-only：非 owner 发起 invite/kick/rename 一律 Err，成员可退群", async () => {
    const self = (await mockBackend.nodeStatus()).peerId ?? (await startNode());
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
    const before = eventsOf("chat_group_state").length;
    const left = await mockBackend.groupLeave(foreign.groupId);
    expect(left.state).toBe("left");
    expect(eventsOf("chat_group_state")).toHaveLength(before + 1);
  });

  it("群主不能退群；invite/kick/rename 各自 rev+1 并发 roster 回执", async () => {
    await startNode();
    const a = await addFriend("g3a");
    const b = await addFriend("g3b");
    const group = await createGroupWith("研发群", [a]);

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
    expect(eventsOf("chat_group_state")).toHaveLength(4); // 建群+invite+kick+rename
  });

  it("群成员上限 32：满员后邀请拒绝", async () => {
    await startNode();
    const filler = Array.from({ length: 31 }, (_, i) => peerId(`fill${i}`));
    const group: GroupJson = {
      groupId: groupOf("cap00000001"),
      name: "大群",
      owner: (await mockBackend.nodeStatus()).peerId!,
      members: [peerId("self-owner"), ...filler],
      rev: 0,
      state: "active",
      tsMs: 1,
    };
    seedMockGroup(group);
    const friend = await addFriend("capfriend");
    await expect(
      mockBackend.groupInvite(group.groupId, [friend]),
    ).rejects.toThrow(/上限/);
  });
});

describe("mock group：发送与事件流", () => {
  it("group_send：在线成员 ack 发 chat_group_status，离线成员保持 pending", async () => {
    await startNode();
    const a = await addFriend("s1a");
    const b = await addFriend("s1b");
    const group = await createGroupWith("事件群", [a, b]);
    await connect(a); // b 离线

    const send = mockBackend.groupSend(group.groupId, "text", "大家好");
    await vi.advanceTimersByTimeAsync(600);
    const report = await send;
    expect(report.recipients).toBe(2);
    expect(report.acked).toBe(1); // 仅 a 在线
    expect(report.delivered).toBe(false);
    expect(report.message.status).toBe("pending");
    expect(report.message.acks).toEqual([a]);
    expect(report.message.senderId).toBe((await mockBackend.nodeStatus()).peerId);

    const statusEvents = eventsOf("chat_group_status");
    expect(statusEvents).toHaveLength(1);
    expect(statusEvents[0]).toMatchObject({
      type: "chat_group_status",
      groupId: group.groupId,
      messageId: report.message.id,
      acks: [a],
      status: "pending",
    });
  });

  it("全员在线送达 delivered=true；文本触发成员回复发 chat_group_message", async () => {
    await startNode();
    const a = await addFriend("s2a");
    const group = await createGroupWith("全通群", [a]);
    await connect(a);

    const send = mockBackend.groupSend(group.groupId, "text", "在吗");
    await vi.advanceTimersByTimeAsync(600);
    const report = await send;
    expect(report.delivered).toBe(true);
    expect(report.message.status).toBe("delivered");
    const statusEvents = eventsOf("chat_group_status");
    expect(statusEvents).toHaveLength(1);
    expect(statusEvents[0]).toMatchObject({ status: "delivered", acks: [a] });

    // 成员 mock 回复：chat_group_message 入站事件可见可测，且落群历史
    await vi.advanceTimersByTimeAsync(500);
    const messages = eventsOf("chat_group_message");
    expect(messages).toHaveLength(1);
    const inbound = (messages[0] as { message: GroupMessageJson }).message;
    expect(inbound.groupId).toBe(group.groupId);
    expect(inbound.senderId).toBe(a);
    expect(inbound.text).toContain("mock 回复");
    const history = await mockBackend.groupHistory(group.groupId, null, 10);
    expect(history.map((m) => m.id)).toContain(inbound.id);
  });

  it("group_send 校验：未知群/非 active/空文本/缺 media 一律 Err", async () => {
    await startNode();
    const a = await addFriend("s3a");
    const group = await createGroupWith("校验群", [a]);
    await expect(
      mockBackend.groupSend(groupOf("missing01"), "text", "hi"),
    ).rejects.toThrow(/群不存在/);
    await expect(
      mockBackend.groupSend(group.groupId, "text", "   "),
    ).rejects.toThrow(/文本消息不能为空/);
    await expect(
      mockBackend.groupSend(group.groupId, "image", undefined),
    ).rejects.toThrow(/必须携带 media/);
    await expect(
      mockBackend.groupSend(group.groupId, "image", undefined, {
        name: "x.png",
        mime: "video/mp4",
        dataBase64: "aGk=",
      }),
    ).rejects.toThrow(/不匹配/);
  });
});

describe("mock group：历史与媒体", () => {
  it("groupHistory 时间 desc + beforeId 游标 + limit 上限", async () => {
    const a = await addFriend("h1a");
    const group = await createGroupWith("历史群", [a]);
    for (const text of ["一", "二", "三"]) {
      injectMockGroupIncoming(group.groupId, {
        senderId: a,
        kind: "text",
        text,
      });
    }
    expect(eventsOf("chat_group_message")).toHaveLength(3);
    const page1 = await mockBackend.groupHistory(group.groupId, null, 2);
    expect(page1.map((m) => m.text)).toEqual(["三", "二"]);
    const page2 = await mockBackend.groupHistory(
      group.groupId,
      page1[1]!.id,
      50,
    );
    expect(page2.map((m) => m.text)).toEqual(["一"]);
    await expect(
      mockBackend.groupHistory(group.groupId, "no-such-id", 10),
    ).rejects.toThrow(/beforeId/);
    await expect(
      mockBackend.groupHistory(group.groupId, null, 0),
    ).rejects.toThrow(/limit/);
  });

  it("groupMediaFile 返回 media/<groupId>/ 路径；非媒体 Err", async () => {
    const a = await addFriend("m1a");
    const group = await createGroupWith("媒体群", [a]);
    const send = mockBackend.groupSend(group.groupId, "image", undefined, {
      name: "截图.png",
      mime: "image/png",
      dataBase64: "aGk=",
    });
    await vi.advanceTimersByTimeAsync(100);
    const report = await send;
    const file = await mockBackend.groupMediaFile(
      group.groupId,
      report.message.id,
    );
    expect(file.mime).toBe("image/png");
    expect(file.path).toContain("media/");
    expect(file.path).toContain(group.groupId);
    expect(file.name).toBe("截图.png");
    await expect(
      mockBackend.groupMediaFile(group.groupId, "no-such-id"),
    ).rejects.toThrow(/不存在或不是媒体/);
  });
});
