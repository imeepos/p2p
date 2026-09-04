import { describe, expect, it } from "vitest";

import {
  base64ByteSize,
  mergeMessages,
  removeLocal,
} from "./chat-local";
import {
  MAX_MEDIA_BYTES,
  fileToChatMedia,
  inferKind,
  kindForMime,
  resolveMime,
} from "./chat-media";
import { createMockChatBackend, type MockChatDeps } from "./mock-chat";
import {
  peerId,
  textMessage,
} from "@/test/chat-boundaries-fixtures";

describe("chat boundary helpers", () => {
  it("preserves unicode and exact text byte estimates while deduplicating乱序消息", () => {
    expect("😀汉字".length).toBe(4);
    expect(base64ByteSize("AAAA")).toBe(3);
    const old = textMessage("old", "p", "旧", { tsMs: 1 });
    const newer = textMessage("new", "p", "新", { tsMs: 3 });
    const replacement = textMessage("old", "p", "更新", { tsMs: 2, status: "failed" });
    expect(mergeMessages([newer, old], [replacement, newer])).toEqual([replacement, newer]);
    expect(removeLocal([old, newer], "missing")).toHaveLength(2);
  });

  it("infers every supported attachment kind and safe file fallback", () => {
    expect(inferKind("photo.PNG", "")).toBe("image");
    expect(inferKind("sound.ogg", "")).toBe("audio");
    expect(inferKind("clip.mov", "")).toBe("video");
    expect(inferKind("document.bin", "")).toBe("file");
    expect(inferKind("wrong.png", "application/octet-stream")).toBe("file");
    expect(inferKind("photo.jpg", "image/jpeg")).toBe("image");
    expect(inferKind("sound.mp4", "audio/mp4")).toBe("audio");
    expect(inferKind("clip.mp4", "video/mp4")).toBe("video");
    expect(inferKind("empty", "")).toBe("file");
    expect(kindForMime("IMAGE/PNG")).toBe("image");
    expect(kindForMime("application/x-unknown")).toBe("file");
    expect(resolveMime("no-extension", "")).toBe("application/octet-stream");
    expect(resolveMime("photo.png", "IMAGE/PNG")).toBe("image/png");
  });

  it("accepts zero-byte files and rejects exactly 64MiB plus one before reading", async () => {
    const empty = new File([], "empty.bin", { type: "application/octet-stream" });
    await expect(fileToChatMedia(empty)).resolves.toMatchObject({ name: "empty.bin", mime: "application/octet-stream" });
    const oversized = Object.create(File.prototype) as File;
    Object.defineProperties(oversized, {
      name: { value: "huge.bin" },
      type: { value: "application/octet-stream" },
      size: { value: MAX_MEDIA_BYTES + 1 },
    });
    await expect(fileToChatMedia(oversized)).rejects.toThrow(/超过单条消息上限/);
  });
});

describe("mock chat contract boundaries", () => {
  const deps: MockChatDeps = {
    emit: () => {},
    selfPeerId: () => peerId("self"),
    isRunning: () => false,
    isConnected: () => false,
    addKnownPeer: () => {},
  };

  it("rejects invalid peer/address, accepts empty nickname and exact 64, rejects 65 and duplicate", async () => {
    const backend = createMockChatBackend(deps);
    const validPeer = peerId("contract-a");
    const secondPeer = peerId("contract-b");
    const thirdPeer = peerId("contract-c");
    await expect(backend.chatFriendAdd("bad", "", [])).rejects.toThrow(/peerId/);
    await expect(backend.chatFriendAdd(validPeer, "n", ["127.0.0.1:1"])).rejects.toThrow(/地址/);
    await expect(backend.chatFriendAdd(validPeer, "x".repeat(65), [])).rejects.toThrow(/nickname/);
    await expect(backend.chatFriendAdd(validPeer, "", [])).resolves.toMatchObject({ nickname: "" });
    await expect(backend.chatFriendAdd(validPeer, "again", [])).rejects.toThrow(/已是好友/);
    await expect(backend.chatFriendAdd(secondPeer, "x".repeat(64), [])).resolves.toMatchObject({ nickname: "x".repeat(64) });
    await expect(backend.chatFriendAdd(thirdPeer, "  ", [])).resolves.toMatchObject({ nickname: "" });
  });

  it("rejects invalid history limits and cursors while preserving empty pages", async () => {
    const backend = createMockChatBackend(deps);
    const peer = peerId("history-a");
    await expect(backend.chatHistory(peer, null, 1)).resolves.toEqual([]);
    await expect(backend.chatHistory(peer, null, 0)).rejects.toThrow(/limit/);
    await expect(backend.chatHistory(peer, null, -1)).rejects.toThrow(/limit/);
    await expect(backend.chatHistory(peer, "missing", 100)).rejects.toThrow(/beforeId/);
    await expect(backend.chatHistory(peer, null, 101)).resolves.toEqual([]);
  });

  it("chatSend: no friend rejected; blank/2001 rejected; unicode 2000 accepted", async () => {
    const backend = createMockChatBackend(deps);
    await expect(backend.chatSend(peerId("stranger"), "text", "你好")).rejects.toThrow(/还不是好友/);
    const peer = peerId("send-text");
    await backend.chatFriendAdd(peer, "", []);
    await expect(backend.chatSend(peer, "text", "   ")).rejects.toThrow(/不能为空/);
    await expect(backend.chatSend(peer, "text", "x".repeat(2001))).rejects.toThrow(/超过 2000/);
    // 😀 为代理对（2 个 UTF-16 单元）：repeat(1000) 恰好 2000 单元，压线通过
    const unicode = "😀".repeat(1000);
    const report = await backend.chatSend(peer, "text", unicode);
    expect(report.delivered).toBe(false);
    expect(report.message.status).toBe("pending");
    expect(report.message.text).toBe(unicode);
  });

  it("chatSend media: mime/kind mismatch rejected, oversized payload rejected", async () => {
    const backend = createMockChatBackend(deps);
    const peer = peerId("send-media");
    await backend.chatFriendAdd(peer, "", []);
    await expect(
      backend.chatSend(peer, "image", undefined, { name: "a.bin", mime: "application/octet-stream", dataBase64: "AAAA" }),
    ).rejects.toThrow(/不匹配/);
    // base64 解码字节数 = floor(len/4)*3：先对字节数向上取整再乘 4，确保解码后 > 64MiB
    const hugeB64 = "A".repeat(Math.ceil((MAX_MEDIA_BYTES + 1) / 3) * 4);
    await expect(
      backend.chatSend(peer, "file", undefined, { name: "huge.bin", mime: "application/octet-stream", dataBase64: hugeB64 }),
    ).rejects.toThrow(/超过单条消息上限/);
  });
});
