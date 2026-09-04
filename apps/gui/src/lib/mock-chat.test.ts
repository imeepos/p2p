import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { mockBackend } from "./mock-ipc";
import { forceMockMessageStatus, injectMockIncoming } from "./mock-chat-inject";
import type { GuiConfig, NodeEventJson } from "./ipc-types";

// mock chat 段（契约 v7 §12）：好友校验/发送状态事件/历史分页/媒体占位。
// state 是模块级单例，测试间共享——每个用例用独立 seed 生成 peerId 隔离。

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

async function stopIfRunning() {
  const status = await mockBackend.nodeStatus();
  if (status.running) {
    const stop = mockBackend.nodeStop();
    await vi.advanceTimersByTimeAsync(500);
    await stop;
  }
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(async () => {
  await stopIfRunning();
  vi.useRealTimers();
});

describe("mock chat：好友簿", () => {
  it("chatFriendAdd 校验 base58/昵称/地址/重复，合法输入入簿", async () => {
    await expect(mockBackend.chatFriendAdd("zzz", "a", [])).rejects.toThrow(
      /peerId 非法/,
    );
    const p = peerId("add-validation");
    await expect(
      mockBackend.chatFriendAdd(p, "x".repeat(65), []),
    ).rejects.toThrow(/nickname/);
    await expect(
      mockBackend.chatFriendAdd(p, "nick", ["192.168.1.5:3400"]),
    ).rejects.toThrow(/地址语法非法/);
    const friend = await mockBackend.chatFriendAdd(p, "nick", [ADDR]);
    expect(friend).toMatchObject({ peerId: p, nickname: "nick", addrs: [ADDR] });
    await expect(mockBackend.chatFriendAdd(p, "again", [])).rejects.toThrow(
      /已是好友/,
    );
  });

  it("chatFriendAdd 拒绝把自己加为好友", async () => {
    const start = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    const status = await start;
    expect(status.peerId).toBeTruthy();
    await expect(
      mockBackend.chatFriendAdd(status.peerId!, "me", []),
    ).rejects.toThrow(/不能把自己/);
  });

  it("chatFriendRemove 幂等且不删消息历史", async () => {
    const p = peerId("remove-keeps-history");
    await mockBackend.chatFriendAdd(p, "a", []);
    const list = await mockBackend.chatFriendsList();
    expect(list.some((f) => f.peerId === p)).toBe(true);

    await mockBackend.chatSend(p, "text", "hello");
    await expect(mockBackend.chatFriendRemove(p)).resolves.toBe(true);
    await expect(mockBackend.chatFriendRemove(p)).resolves.toBe(false);

    const history = await mockBackend.chatHistory(p);
    expect(history).toHaveLength(1);
    expect(history[0]).toMatchObject({ sender: "me", text: "hello" });
  });
});

describe("mock chat：发送校验", () => {
  it("非好友/空文本/超长文本/kind 与 media 缺配均 Err", async () => {
    const p = peerId("send-validation");
    await expect(mockBackend.chatSend(p, "text", "hi")).rejects.toThrow(
      /还不是好友/,
    );
    await mockBackend.chatFriendAdd(p, "", []);
    await expect(mockBackend.chatSend(p, "text", "   ")).rejects.toThrow(
      /不能为空/,
    );
    await expect(
      mockBackend.chatSend(p, "text", "x".repeat(2001)),
    ).rejects.toThrow(/2000/);
    await expect(mockBackend.chatSend(p, "image")).rejects.toThrow(
      /必须携带 media/,
    );
    await expect(
      mockBackend.chatSend(p, "image", undefined, {
        name: "a.mp3",
        mime: "audio/mpeg",
        dataBase64: "AAAA",
      }),
    ).rejects.toThrow(/不匹配/);
    await expect(
      mockBackend.chatSend(p, "file", undefined, {
        name: "a.png",
        mime: "image/png",
        dataBase64: "AAAA",
      }),
    ).rejects.toThrow(/不匹配/);
    await expect(
      mockBackend.chatSend(p, "image", undefined, {
        name: "a.png",
        mime: "image/png",
        dataBase64: "!!!",
      }),
    ).rejects.toThrow(/base64/);
  });

  it("附件解码后超 64MiB 返回 Err", async () => {
    const p = peerId("oversize-media");
    await mockBackend.chatFriendAdd(p, "", []);
    const huge = "A".repeat(Math.ceil(((64 * 1024 * 1024 + 4) / 3) * 4));
    await expect(
      mockBackend.chatSend(p, "file", undefined, {
        name: "big.bin",
        mime: "application/octet-stream",
        dataBase64: huge,
      }),
    ).rejects.toThrow(/上限/);
  });

  // IM-T46B：replyTo 与真实后端同语义——提供须非空字符串，原样入库；缺省无引用。
  it("replyTo 透传：入库携带引用 id，空白引用拒绝", async () => {
    const p = peerId("reply-passthrough");
    await mockBackend.chatFriendAdd(p, "", []);
    await expect(
      mockBackend.chatSend(p, "text", "hi", undefined, "   "),
    ).rejects.toThrow(/回复引用非法/);
    const quoted = await mockBackend.chatSend(p, "text", "reply", undefined, "target-1");
    expect(quoted.message.replyTo).toBe("target-1");
    const plain = await mockBackend.chatSend(p, "text", "plain");
    expect(plain.message.replyTo ?? null).toBeNull();
    const history = await mockBackend.chatHistory(p);
    expect(history.find((m) => m.id === quoted.message.id)?.replyTo).toBe("target-1");
  });
});

describe("mock chat：发送与历史", () => {
  it("在线好友送达：chat_status(sent→delivered) + delivered 报告 + mock 回复", async () => {
    const p = peerId("delivered-path");
    await mockBackend.chatFriendAdd(p, "buddy", [ADDR]);
    const start = mockBackend.nodeStart(CFG);
    await vi.advanceTimersByTimeAsync(1000);
    await start;
    const connect = mockBackend.peerConnect(p);
    await vi.advanceTimersByTimeAsync(1000);
    expect((await connect).ok).toBe(true);

    const events: NodeEventJson[] = [];
    const unlisten = await mockBackend.onNodeEvent((e) => {
      if (e.type === "chat_message" || e.type === "chat_status") events.push(e);
    });

    const send = mockBackend.chatSend(p, "text", "hello mock");
    await vi.advanceTimersByTimeAsync(120);
    await vi.advanceTimersByTimeAsync(200);
    const report = await send;
    expect(report.delivered).toBe(true);
    expect(report.message.status).toBe("delivered");

    await vi.advanceTimersByTimeAsync(400);
    const statuses = events
      .filter((e) => e.type === "chat_status")
      .map((e) => (e as { status: string }).status);
    expect(statuses).toEqual(["sent", "delivered"]);
    const inbound = events.find((e) => e.type === "chat_message") as
      | { message: { sender: string } }
      | undefined;
    expect(inbound?.message.sender).toBe("them");

    const history = await mockBackend.chatHistory(p);
    expect(history.map((m) => m.sender)).toEqual(["them", "me"]);
    unlisten();
  });

  it("离线好友发送保持 pending（outbox 语义）", async () => {
    const p = peerId("offline-pending");
    await mockBackend.chatFriendAdd(p, "", []);
    const report = await mockBackend.chatSend(p, "text", "queued");
    expect(report).toMatchObject({ delivered: false });
    expect(report.message.status).toBe("pending");
  });

  it("chatHistory 分页：desc 顺序、limit 截断、beforeId 严格更早", async () => {
    const p = peerId("history-paging");
    await mockBackend.chatFriendAdd(p, "", []);
    for (const t of ["one", "two", "three"]) {
      await mockBackend.chatSend(p, "text", t);
    }
    const page = await mockBackend.chatHistory(p);
    expect(page.map((m) => m.text)).toEqual(["three", "two", "one"]);

    const top2 = await mockBackend.chatHistory(p, null, 2);
    expect(top2.map((m) => m.text)).toEqual(["three", "two"]);

    const beforeTwo = await mockBackend.chatHistory(p, page[1]!.id);
    expect(beforeTwo.map((m) => m.text)).toEqual(["one"]);

    await expect(mockBackend.chatHistory(p, "no-such-id")).rejects.toThrow(
      /beforeId/,
    );
    await expect(mockBackend.chatHistory(p, null, 0)).rejects.toThrow(/limit/);
    await expect(mockBackend.chatHistory(p, null, 101)).resolves.toHaveLength(3);
  });

  it("媒体消息：path 占位含 sanitize 文件名，chatMediaFile 回读，文本消息 Err", async () => {
    const p = peerId("media-path");
    await mockBackend.chatFriendAdd(p, "", []);
    const report = await mockBackend.chatSend(p, "image", undefined, {
      name: "照片 pic.png",
      mime: "image/png",
      dataBase64: "iVBORw0KGgo=",
    });
    expect(report.message.media).toMatchObject({
      name: "照片 pic.png",
      mime: "image/png",
      size: 8,
    });
    expect(report.message.media?.path).toContain(`/chat/media/${p}/`);
    expect(report.message.media?.path).toMatch(/_[A-Za-z0-9._-]+$/);

    const file = await mockBackend.chatMediaFile(p, report.message.id);
    expect(file).toMatchObject({ mime: "image/png", name: "照片 pic.png" });
    expect(file.path).toBe(report.message.media?.path);

    await expect(mockBackend.chatMediaFile(p, "missing")).rejects.toThrow(
      /不存在/,
    );
    const textReport = await mockBackend.chatSend(p, "text", "plain");
    await expect(
      mockBackend.chatMediaFile(p, textReport.message.id),
    ).rejects.toThrow(/不是媒体消息/);
  });
});
describe("mock chat：场景注入（IM-T50）", () => {
  it("injectMockIncoming 注入 them 五类型消息并发 chat_message 事件", async () => {
    const p = peerId("inject-them");
    const events: NodeEventJson[] = [];
    const unlisten = await mockBackend.onNodeEvent((e) => events.push(e));

    const t1 = injectMockIncoming(p, { kind: "text", text: "对方文本" });
    expect(t1).toMatchObject({ sender: "them", kind: "text", text: "对方文本", status: "delivered" });
    for (const kind of ["image", "audio", "video", "file"] as const) {
      const m = injectMockIncoming(p, {
        kind,
        media: { name: `demo-${kind}.bin`, mime: "application/octet-stream", dataBase64: "aGk=" },
      });
      expect(m).toMatchObject({ sender: "them", kind });
      expect(m.media).toMatchObject({ name: `demo-${kind}.bin`, size: 2 });
    }

    expect(events.filter((e) => e.type === "chat_message")).toHaveLength(5);
    const history = await mockBackend.chatHistory(p, null, 10);
    expect(history).toHaveLength(5);
    expect(history.every((m) => m.sender === "them")).toBe(true);
    await unlisten();
  });

  it("注入校验：text 缺文本 / 媒体缺 media 显式报错", () => {
    const p = peerId("inject-invalid");
    expect(() => injectMockIncoming(p, { kind: "text" })).toThrow(/需要非空 text/);
    expect(() => injectMockIncoming(p, { kind: "image" })).toThrow(/需要 media/);
  });

  it("forceMockMessageStatus 推进 pending→failed 并改历史态；目标缺失报错", async () => {
    const p = peerId("inject-status");
    await mockBackend.chatFriendAdd(p, "", []);
    const events: NodeEventJson[] = [];
    const unlisten = await mockBackend.onNodeEvent((e) => events.push(e));

    const sent = await mockBackend.chatSend(p, "text", "我的消息");
    expect(sent.message.status).toBe("pending");
    const forced = forceMockMessageStatus(p, sent.message.id, "failed");
    expect(forced.status).toBe("failed");
    const statusEvents = events.filter((e) => e.type === "chat_status");
    expect(statusEvents).toHaveLength(1);
    expect(statusEvents[0]).toMatchObject({ peer: p, messageId: sent.message.id, status: "failed" });

    const history = await mockBackend.chatHistory(p, null, 10);
    expect(history.find((m) => m.id === sent.message.id)?.status).toBe("failed");
    expect(() => forceMockMessageStatus(p, "no-such-id", "failed")).toThrow(/不存在/);
    await unlisten();
  });
});
