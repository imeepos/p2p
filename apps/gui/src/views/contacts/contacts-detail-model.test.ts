// 资料卡选中模型（双栏改版）纯函数单测：选中解析、失效回退、key 稳定性。
import { describe, expect, it } from "vitest";

import type { AcpEndpoint } from "@/acp/protocol";
import type { ChatFriendJson, FriendInviteJson, GroupJson } from "@/lib/ipc-types";

import {
  firstAvailableEntity,
  resolveEntity,
  selectionKey,
  type ContactsData,
} from "./contacts-detail-model";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER_B = "2jSUsWcEf7z68xBscf2YmVYzQ4uPZfpMz8XRW3vruJU4";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null, group: null };
}

function groupOf(groupId: string, name: string, owner: string, state: "active" | "kicked"): GroupJson {
  return { groupId, name, owner, members: [owner], rev: 1, state, tsMs: 1_000 };
}

function endpointOf(endpointId: string): AcpEndpoint {
  return { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: PEER_B, alias: endpointId, endpointId };
}

function inviteOf(peerId: string, direction: "in" | "out"): FriendInviteJson {
  return { peerId, nickname: "对方", addrs: [], note: null, direction, tsMs: 1, delivered: true };
}

const EMPTY: ContactsData = { friends: [], invites: [], groups: [], agents: [] };

describe("selectionKey", () => {
  it("四类实体 key 互不冲突且稳定", () => {
    expect(selectionKey({ kind: "friend", peerId: PEER })).toBe("friend:" + PEER);
    expect(selectionKey({ kind: "group", groupId: "g1" })).toBe("group:g1");
    expect(selectionKey({ kind: "agent", endpointId: "ep-1" })).toBe("agent:ep-1");
    expect(selectionKey({ kind: "invite", peerId: PEER, direction: "in" })).toBe(
      "invite:in:" + PEER,
    );
    expect(selectionKey({ kind: "invite", peerId: PEER, direction: "out" })).not.toBe(
      selectionKey({ kind: "invite", peerId: PEER, direction: "in" }),
    );
  });
});

describe("resolveEntity", () => {
  it("好友命中与失效（已删除）", () => {
    const data: ContactsData = { ...EMPTY, friends: [friendOf(PEER, "小圆")] };
    expect(resolveEntity({ kind: "friend", peerId: PEER }, data)?.friend?.nickname).toBe("小圆");
    expect(resolveEntity({ kind: "friend", peerId: PEER_B }, data)).toBeNull();
  });

  it("非 active 群不作为有效选中（与通讯录行同口径）", () => {
    const data: ContactsData = { ...EMPTY, groups: [groupOf("g-k", "被踢群", PEER_B, "kicked")] };
    expect(resolveEntity({ kind: "group", groupId: "g-k" }, data)).toBeNull();
    const active: ContactsData = { ...EMPTY, groups: [groupOf("g-a", "群", PEER_B, "active")] };
    expect(resolveEntity({ kind: "group", groupId: "g-a" }, active)?.group?.groupId).toBe("g-a");
  });

  it("agent 以 endpointId ?? wsUrl 为准；invite 按 peer + direction 双键匹配", () => {
    const data: ContactsData = {
      ...EMPTY,
      agents: [endpointOf("ep-1")],
      invites: [inviteOf(PEER, "out")],
    };
    expect(resolveEntity({ kind: "agent", endpointId: "ep-1" }, data)?.agent?.alias).toBe("ep-1");
    expect(resolveEntity({ kind: "agent", endpointId: "ep-2" }, data)).toBeNull();
    expect(resolveEntity({ kind: "invite", peerId: PEER, direction: "out" }, data)?.invite).toBeTruthy();
    expect(resolveEntity({ kind: "invite", peerId: PEER, direction: "in" }, data)).toBeNull();
  });
});

describe("firstAvailableEntity", () => {
  it("回退顺序：好友 → 群 → Agent → 邀请（in 优先于 out）", () => {
    expect(firstAvailableEntity(EMPTY)).toBeNull();
    const onlyInvite: ContactsData = {
      ...EMPTY,
      invites: [inviteOf(PEER, "out"), inviteOf(PEER_B, "in")],
    };
    const picked = firstAvailableEntity(onlyInvite);
    expect(picked?.selection.kind).toBe("invite");
    expect(picked?.selection.kind === "invite" && picked.selection.direction).toBe("in");
    const withFriend: ContactsData = { ...onlyInvite, friends: [friendOf(PEER, "小圆")] };
    expect(firstAvailableEntity(withFriend)?.selection.kind).toBe("friend");
    const withGroup: ContactsData = { ...onlyInvite, groups: [groupOf("g1", "群", PEER, "active")] };
    expect(firstAvailableEntity(withGroup)?.selection.kind).toBe("group");
    const withAgent: ContactsData = { ...onlyInvite, agents: [endpointOf("ep-1")] };
    expect(firstAvailableEntity(withAgent)?.selection.kind).toBe("agent");
  });
});
