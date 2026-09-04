import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatMessageJson, ChatSendReport } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    send: vi.fn<
      (
        peer: string,
        kind: string,
        text?: string,
        media?: unknown,
        replyTo?: string,
      ) => Promise<ChatSendReport>
    >(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>
    >(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { chatSend: mocks.send, chatHistory: mocks.history },
}));

import { useChatStore } from "./chat-store";

const PEER = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWX";

function text(id: string, content: string, tsMs: number): ChatMessageJson {
  return { id, peer: PEER, sender: "me", kind: "text", tsMs, text: content, media: null, status: "delivered" };
}

function media(id: string, name: string, tsMs: number, status: ChatMessageJson["status"]): ChatMessageJson {
  return {
    id,
    peer: PEER,
    sender: "me",
    kind: "file",
    tsMs,
    text: null,
    media: { name, mime: "application/octet-stream", size: 10, path: null },
    status,
  };
}

function seed(messages: ChatMessageJson[], last: ChatMessageJson | null): void {
  useChatStore.setState((s) => ({
    messagesByPeer: { ...s.messagesByPeer, [PEER]: messages },
    lastMessageByPeer: { ...s.lastMessageByPeer, [PEER]: last },
  }));
}

beforeEach(() => {
  mocks.send.mockReset();
  mocks.history.mockReset();
  useChatStore.setState({
    friends: [],
    friendsLoaded: false,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
});

describe("chat-store 历史加载错误态（IM-T50）", () => {
  it("selectPeer 失败落 historyError；重试成功清除并载入消息", async () => {
    mocks.history.mockRejectedValueOnce(new Error("history db locked"));
    await expect(useChatStore.getState().selectPeer(PEER)).rejects.toThrow(
      "history db locked",
    );
    expect(useChatStore.getState().historyError[PEER]).toBe("history db locked");

    mocks.history.mockResolvedValueOnce([text("m1", "早", 1000)]);
    await useChatStore.getState().selectPeer(PEER);
    expect(useChatStore.getState().historyError[PEER] ?? null).toBeNull();
    expect(useChatStore.getState().historyLoaded[PEER]).toBe(true);
    expect(
      (useChatStore.getState().messagesByPeer[PEER] ?? []).map((m) => m.id),
    ).toEqual(["m1"]);
  });

  it("loadOlder 失败落 olderError；成功后清除且旧页并入", async () => {
    mocks.history
      .mockResolvedValueOnce(Array.from({ length: 50 }, (_, i) => text(`m${i}`, "页", i + 1)))
      .mockRejectedValueOnce(new Error("older boom"))
      .mockResolvedValueOnce([text("old", "更早", 0)]);
    await useChatStore.getState().selectPeer(PEER);
    expect(useChatStore.getState().hasMore[PEER]).toBe(true);

    await expect(useChatStore.getState().loadOlder(PEER)).rejects.toThrow("older boom");
    expect(useChatStore.getState().olderError[PEER]).toBe("older boom");

    await useChatStore.getState().loadOlder(PEER);
    expect(useChatStore.getState().olderError[PEER] ?? null).toBeNull();
    expect(
      (useChatStore.getState().messagesByPeer[PEER] ?? []).some((m) => m.id === "old"),
    ).toBe(true);
  });
});

describe("chat-store 占位回滚与会话摘要", () => {
  it("媒体发送失败：占位移除且摘要回退为上一条真实消息（IM-T48 裁决项）", async () => {
    const m1 = text("m1", "早", 1000);
    seed([m1], m1);
    mocks.send.mockRejectedValue(new Error("boom"));
    await expect(
      useChatStore.getState().sendMedia(PEER, "file", { name: "录音-final-v2.m4a", mime: "audio/mp4", dataBase64: "aGk=" }),
    ).rejects.toThrow("boom");
    const list = useChatStore.getState().messagesByPeer[PEER] ?? [];
    expect(list.some((m) => m.id.startsWith("local-"))).toBe(false);
    expect(useChatStore.getState().lastMessageByPeer[PEER]?.id).toBe("m1");
  });

  it("发送失败且无历史：占位移除且摘要清空", async () => {
    seed([], null);
    mocks.send.mockRejectedValue(new Error("boom"));
    await expect(useChatStore.getState().sendText(PEER, "你好")).rejects.toThrow("boom");
    expect((useChatStore.getState().messagesByPeer[PEER] ?? []).length).toBe(0);
    expect(useChatStore.getState().lastMessageByPeer[PEER] ?? null).toBeNull();
  });

  it("发送在途对端来新消息：回滚不覆盖事件写入的摘要", async () => {
    const m1 = text("m1", "早", 1000);
    const incoming = text("incoming-1", "对方插话", 2000);
    seed([m1], m1);
    mocks.send.mockImplementation(async () => {
      useChatStore.setState((s) => ({
        lastMessageByPeer: { ...s.lastMessageByPeer, [PEER]: incoming },
      }));
      throw new Error("boom");
    });
    await expect(useChatStore.getState().sendText(PEER, "你好")).rejects.toThrow("boom");
    expect(useChatStore.getState().lastMessageByPeer[PEER]?.id).toBe("incoming-1");
  });

  it("取消末尾占位附件：摘要同步回退；摘要已指向新消息则保持", async () => {
    const m1 = text("m1", "早", 1000);
    seed([m1], m1);
    const placeholder = media("local-1", "demo.png", 3000, "pending");
    seed([m1, placeholder], placeholder);
    useChatStore.getState().cancelPending(PEER, "local-1");
    expect((useChatStore.getState().messagesByPeer[PEER] ?? []).length).toBe(1);
    expect(useChatStore.getState().lastMessageByPeer[PEER]?.id).toBe("m1");

    const newer = text("newer", "新消息", 4000);
    seed([m1, newer], newer);
    useChatStore.getState().cancelPending(PEER, "local-2");
    expect(useChatStore.getState().lastMessageByPeer[PEER]?.id).toBe("newer");
  });
});
