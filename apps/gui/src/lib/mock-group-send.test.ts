import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import { injectMockGroupIncoming } from "./mock-group-inject";
import {
  addFriend,
  collectGroupEvents,
  connect,
  groupOf,
  startNode,
  stopIfRunning,
} from "./mock-group-test-utils";
import type { GroupJson, GroupMessageJson } from "./ipc-types";

// mock 群发送面（im-group-design §6）：fan-out ack 事件流/delivered 推进/
// 成员 mock 回复（chat_group_message）/历史分页/媒体路径。
// roster 校验语义见 mock-group.test.ts。

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

describe("mock group：发送与事件流", () => {
  it("group_send：在线成员 ack 发 chat_group_status，离线成员保持 pending", async () => {
    await startNode();
    const a = await addFriend("s1a");
    const b = await addFriend("s1b");
    const group = await mockBackend.groupCreate("事件群", [a, b]);
    await connect(a); // b 离线

    const send = mockBackend.groupSend(group.groupId, "text", "大家好");
    await vi.advanceTimersByTimeAsync(600);
    const report = await send;
    expect(report.recipients).toBe(2);
    expect(report.acked).toBe(1); // 仅 a 在线
    expect(report.delivered).toBe(false);
    expect(report.message.status).toBe("pending");
    expect(report.message.acks).toEqual([a]);

    const statusEvents = bus.eventsOf("chat_group_status");
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
    const group = await mockBackend.groupCreate("全通群", [a]);
    await connect(a);

    const send = mockBackend.groupSend(group.groupId, "text", "在吗");
    await vi.advanceTimersByTimeAsync(600);
    const report = await send;
    expect(report.delivered).toBe(true);
    expect(report.message.status).toBe("delivered");
    const statusEvents = bus.eventsOf("chat_group_status");
    expect(statusEvents).toHaveLength(1);
    expect(statusEvents[0]).toMatchObject({ status: "delivered", acks: [a] });

    // 成员 mock 回复：chat_group_message 入站事件可见可测，且落群历史
    await vi.advanceTimersByTimeAsync(500);
    const messages = bus.eventsOf("chat_group_message");
    expect(messages).toHaveLength(1);
    const inbound = (messages[0] as { message: GroupMessageJson }).message;
    expect(inbound.groupId).toBe(group.groupId);
    expect(inbound.senderId).toBe(a);
    expect(inbound.text).toContain("mock 回复");
    const history = await mockBackend.groupHistory(group.groupId, null, 10);
    expect(history.map((m) => m.id)).toContain(inbound.id);
  });

  it("group_send 校验：未知群/空文本/缺 media/mime 不匹配一律 Err", async () => {
    await startNode();
    const a = await addFriend("s3a");
    const group = await mockBackend.groupCreate("校验群", [a]);
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
    const group: GroupJson = await mockBackend.groupCreate("历史群", [a]);
    for (const text of ["一", "二", "三"]) {
      injectMockGroupIncoming(group.groupId, {
        senderId: a,
        kind: "text",
        text,
      });
    }
    expect(bus.eventsOf("chat_group_message")).toHaveLength(3);
    const page1 = await mockBackend.groupHistory(group.groupId, null, 2);
    expect(page1.map((m) => m.text)).toEqual(["三", "二"]);
    const page2 = await mockBackend.groupHistory(group.groupId, page1[1]!.id, 50);
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
    const group = await mockBackend.groupCreate("媒体群", [a]);
    const send = mockBackend.groupSend(group.groupId, "image", undefined, {
      name: "截图.png",
      mime: "image/png",
      dataBase64: "aGk=",
    });
    await vi.advanceTimersByTimeAsync(100);
    const report = await send;
    const file = await mockBackend.groupMediaFile(group.groupId, report.message.id);
    expect(file.mime).toBe("image/png");
    expect(file.path).toContain("media/");
    expect(file.path).toContain(group.groupId);
    expect(file.name).toBe("截图.png");
    await expect(
      mockBackend.groupMediaFile(group.groupId, "no-such-id"),
    ).rejects.toThrow(/不存在或不是媒体/);
  });
});
