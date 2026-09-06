import { describe, expect, it } from "vitest";
import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";

import { matchInviteForMessage } from "./group-invite-match";

// IMC3 卡片匹配纯函数：方向 + 对端精确优先，同方向最新兜底。

function cardMessage(patch: Partial<ChatMessageJson> = {}): ChatMessageJson {
  return {
    id: "m1",
    peer: "peer-b",
    sender: "them",
    kind: "groupInvite",
    tsMs: 1000,
    text: null,
    media: null,
    status: "delivered",
    replyTo: null,
    groupInvite: { groupId: "g-1", groupName: "项目组", inviterNickname: "阿北", note: null },
    ...patch,
  };
}

function inviteOf(patch: Partial<GroupInviteJson>): GroupInviteJson {
  return {
    id: "inv-1",
    groupId: "g-1",
    groupName: "项目组",
    owner: "owner",
    inviter: "peer-b",
    invitee: "self",
    note: null,
    direction: "in",
    state: "pending",
    tsMs: 1000,
    delivered: true,
    ...patch,
  };
}

describe("matchInviteForMessage", () => {
  it("无消息体（kind 数据缺失）返回 null 不崩", () => {
    const message = cardMessage({ groupInvite: null });
    expect(matchInviteForMessage(message, [inviteOf({})])).toBeNull();
  });

  it("them 卡按 in 向 + inviter 对端精确匹配", () => {
    const invites = [
      inviteOf({ id: "other-group", groupId: "g-2" }),
      inviteOf({ id: "hit" }),
      inviteOf({ id: "out-noise", direction: "out", invitee: "peer-b" }),
    ];
    expect(matchInviteForMessage(cardMessage(), invites)?.id).toBe("hit");
  });

  it("me 卡按 out 向 + invitee 对端精确匹配", () => {
    const message = cardMessage({ sender: "me" });
    const invites = [
      inviteOf({ id: "hit-out", direction: "out", inviter: "self", invitee: "peer-b" }),
      inviteOf({ id: "in-noise" }),
    ];
    expect(matchInviteForMessage(message, invites)?.id).toBe("hit-out");
  });

  it("对端不匹配时回退同方向最新一条", () => {
    const message = cardMessage({ peer: "peer-stranger" });
    const invites = [
      inviteOf({ id: "old", tsMs: 1 }),
      inviteOf({ id: "newest", tsMs: 9 }),
    ];
    expect(matchInviteForMessage(message, invites)?.id).toBe("newest");
  });

  it("群不匹配返回 null", () => {
    expect(matchInviteForMessage(cardMessage(), [inviteOf({ groupId: "g-9" })])).toBeNull();
  });
});
