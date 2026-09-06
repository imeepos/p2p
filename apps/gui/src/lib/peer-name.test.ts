import { describe, expect, it } from "vitest";

import type { ChatFriendJson } from "@/lib/ipc-types";
import { peerDisplayLabel, peerKnownName, shortPeerId } from "./peer-name";

// 44 位典型 base58 长度（契约 §6），覆盖真实缩略路径
const PEER = "a".repeat(40) + "Zzzz";

function friend(over: Partial<ChatFriendJson> = {}): ChatFriendJson {
  return { peerId: PEER, nickname: "", addrs: [], ...over };
}

describe("shortPeerId 截断规则（F02/F21 同口径）", () => {
  it("44 位缩略为前 6 后 4，中间省略号", () => {
    expect(shortPeerId(PEER)).toBe("aaaaaa…Zzzz");
  });

  it("不超过前 6 后 4 总长的原样返回", () => {
    expect(shortPeerId("peer-1")).toBe("peer-1");
    expect(shortPeerId("0123456789")).toBe("0123456789");
  });

  it("超出即截断，即使只超 1 位", () => {
    expect(shortPeerId("0123456789A")).toBe("012345…789A");
  });
});

describe("peerKnownName 昵称/备注优先级（F02）", () => {
  it("在册好友取昵称", () => {
    expect(peerKnownName(PEER, [friend({ nickname: "阿北" })])).toBe("阿北");
  });

  it("昵称为空回退备注", () => {
    expect(peerKnownName(PEER, [friend({ note: "老朋友" })])).toBe("老朋友");
  });

  it("昵称优先于备注", () => {
    expect(
      peerKnownName(PEER, [friend({ nickname: "阿北", note: "老朋友" })]),
    ).toBe("阿北");
  });

  it("两者皆空与非好友均返回 null（调用方回退缩略 ID）", () => {
    expect(peerKnownName(PEER, [friend()])).toBeNull();
    expect(peerKnownName(PEER, [friend({ note: null })])).toBeNull();
    expect(peerKnownName(PEER, [])).toBeNull();
  });
});

describe("peerDisplayLabel 单行标签（事件流）", () => {
  it("好友显示「名字 (缩略ID)」", () => {
    expect(peerDisplayLabel(PEER, [friend({ nickname: "阿北" })])).toBe(
      "阿北 (aaaaaa…Zzzz)",
    );
  });

  it("非好友仅缩略 ID", () => {
    expect(peerDisplayLabel(PEER, [])).toBe("aaaaaa…Zzzz");
  });
});
