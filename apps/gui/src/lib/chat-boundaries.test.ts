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

describe("chat boundary helpers", () => {
  it("preserves unicode and exact text byte estimates while deduplicating乱序消息", () => {
    expect("😀汉字".length).toBe(4);
    expect(base64ByteSize("AAAA")).toBe(3);
    const old = { id: "old", peer: "p", sender: "them" as const, kind: "text" as const, tsMs: 1, text: "旧", media: null, status: "delivered" as const };
    const newer = { ...old, id: "new", tsMs: 3, text: "新" };
    const replacement = { ...old, id: "old", tsMs: 2, text: "更新", status: "failed" as const };
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
  const validPeer = "3xY9" + "1ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".repeat(2).slice(0, 40);
  const secondPeer = "5aA7" + "1ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".repeat(2).slice(0, 40);
  const deps = {
    emit: () => {}, selfPeerId: () => "4zZ8" + "1ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".repeat(2).slice(0, 40),
    isRunning: () => false, isConnected: () => false, addKnownPeer: () => {},
  };

  it("rejects invalid peer/address, accepts empty nickname and exact 64, rejects 65 and duplicate", async () => {
    const backend = (await import("./mock-chat")).createMockChatBackend(deps);
    await expect(backend.chatFriendAdd("bad", "", [])).rejects.toThrow(/peerId/);
    await expect(backend.chatFriendAdd(validPeer, "n", ["127.0.0.1:1"])).rejects.toThrow(/地址/);
    await expect(backend.chatFriendAdd(validPeer, "x".repeat(65), [])).rejects.toThrow(/nickname/);
    await expect(backend.chatFriendAdd(validPeer, " ", ["127.0.0.1/u1"])).resolves.toMatchObject({ nickname: "" });
    await expect(backend.chatFriendAdd(validPeer, "again", [])).rejects.toThrow(/已是好友/);
    await expect(backend.chatFriendAdd(secondPeer, "x".repeat(64), [])).resolves.toMatchObject({ nickname: "x".repeat(64) });
  });

  it("rejects invalid history limits and cursors while preserving empty pages", async () => {
    const backend = (await import("./mock-chat")).createMockChatBackend({ ...deps, isRunning: () => false });
    const peer = validPeer + "A";
    await expect(backend.chatHistory(peer, null, 1)).resolves.toEqual([]);
    await expect(backend.chatHistory(peer, null, 0)).rejects.toThrow(/limit/);
    await expect(backend.chatHistory(peer, null, -1)).rejects.toThrow(/limit/);
    await expect(backend.chatHistory(peer, "missing", 100)).rejects.toThrow(/beforeId/);
    await expect(backend.chatHistory(peer, null, 101)).resolves.toEqual([]);
  });
});
