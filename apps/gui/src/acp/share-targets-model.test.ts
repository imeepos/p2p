// 分享发送目标模型测试：目标合成、选中切换、发送结果归并。
import { describe, expect, it } from "vitest";

import {
  buildTargets,
  summarizeSend,
  targetKey,
  toggleSelected,
} from "./share-targets-model";

const FRIEND = { peerId: "peerAaaaaaaaaa", nickname: "小明", addrs: [] };
const GROUP = {
  groupId: "g-1",
  name: "p2p 群",
  owner: "peerOwner",
  members: ["peerOwner"],
  rev: 1,
  state: "active" as const,
  tsMs: 0,
};

describe("buildTargets 好友与群聊合成", () => {
  it("好友在前群聊在后，键带命名空间，退群/解散群不出现", () => {
    const targets = buildTargets([FRIEND], [GROUP, { ...GROUP, groupId: "g-2", state: "left" }]);
    expect(targets.map((x) => x.key)).toEqual(["friend:peerAaaaaaaaaa", "group:g-1"]);
    expect(targets[0].label).toBe("小明");
    expect(targets[1].label).toBe("p2p 群");
  });

  it("好友空昵称回退 PeerId 缩略", () => {
    const targets = buildTargets([{ ...FRIEND, nickname: "" }], []);
    expect(targets[0].label).toBe(FRIEND.peerId.slice(0, 8));
  });
});

describe("toggleSelected 选中集合", () => {
  it("未选加入、已选移除", () => {
    const key = targetKey("friend", "p1");
    expect(toggleSelected([], key)).toEqual([key]);
    expect(toggleSelected([key], key)).toEqual([]);
  });
});

describe("summarizeSend 结果归并", () => {
  it("全成功/部分失败计数", () => {
    expect(summarizeSend([{ key: "a", ok: true }, { key: "b", ok: true }])).toEqual({
      sent: 2,
      failed: 0,
    });
    expect(summarizeSend([{ key: "a", ok: true }, { key: "b", ok: false }])).toEqual({
      sent: 1,
      failed: 1,
    });
  });
});
