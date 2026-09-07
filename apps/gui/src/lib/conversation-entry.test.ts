import { describe, expect, it } from "vitest";

import type { ChatFriendJson, ChatMessageJson, GroupJson, GroupMessageJson } from "@/lib/ipc-types";
import {
  agentEntry,
  AGENT_PREVIEW_MAX_CHARS,
  filterEntries,
  formatUnreadCount,
  friendEntry,
  groupEntry,
  previewOfMessage,
  sortEntries,
  visibleGroups,
  wsHostOf,
  type ConversationEntry,
  type PreviewLabels,
} from "./conversation-entry";

const LABELS: PreviewLabels = {
  image: "[图片]",
  audio: "[语音]",
  video: "[视频]",
  file: "[文件]",
  self: "我",
};

function msg(kind: ChatMessageJson["kind"], overrides: Partial<ChatMessageJson> = {}): ChatMessageJson {
  return {
    id: "m1",
    peer: "p1",
    sender: "them",
    kind,
    tsMs: 1000,
    text: null,
    media: null,
    status: "delivered",
    ...overrides,
  };
}

describe("previewOfMessage 五种 kind 预览规则（§2.2）", () => {
  it("text 正文单行截断", () => {
    expect(previewOfMessage(msg("text", { text: "你好\n世界" }), LABELS)).toBe("你好 世界");
    const long = "x".repeat(200);
    expect(previewOfMessage(msg("text", { text: long }), LABELS)).toBe("x".repeat(80) + "…");
  });

  it("image/audio/video 显类型词条，file 显词条 + 文件名", () => {
    expect(previewOfMessage(msg("image"), LABELS)).toBe("[图片]");
    expect(previewOfMessage(msg("audio"), LABELS)).toBe("[语音]");
    expect(previewOfMessage(msg("video"), LABELS)).toBe("[视频]");
    expect(
      previewOfMessage(
        msg("file", { media: { name: "a.pdf", mime: "application/pdf", size: 1 } }),
        LABELS,
      ),
    ).toBe("[文件] a.pdf");
  });

  it("无消息返回 null", () => {
    expect(previewOfMessage(null, LABELS)).toBeNull();
  });
});

function friend(overrides: Partial<ChatFriendJson> = {}): ChatFriendJson {
  return { peerId: "PEERAAAA1", nickname: "小圆", addrs: [], note: null, ...overrides };
}

describe("friendEntry 字段对齐（§2.2）", () => {
  it("昵称空回退 PeerId 缩略；note 进 subtitle；本端消息呈 sendState", () => {
    const e = friendEntry({
      friend: friend({ nickname: "", note: "同事" }),
      last: msg("text", { text: "在吗", sender: "me", status: "failed" }),
      unread: 2,
      joinSeq: 1,
      labels: LABELS,
    });
    expect(e.kind).toBe("friend");
    expect(e.title).toBe("PEERAAAA1".slice(0, 8));
    expect(e.subtitle).toBe("同事");
    expect(e.kindMark.initial).toBe(e.title[0]);
    expect(e.kindMark.groupBadge).toBe(false);
    expect(e.lastPreview).toBe("在吗");
    expect(e.sendState).toBe("failed");
  });

  it("最近一条为对方消息时无 sendState；无消息 ts=0 落加入序", () => {
    const e = friendEntry({
      friend: friend(),
      last: msg("text", { sender: "them" }),
      unread: 0,
      joinSeq: 0,
      labels: LABELS,
    });
    expect(e.sendState).toBeNull();
    const noMsg = friendEntry({ friend: friend(), last: null, unread: 0, joinSeq: 3, labels: LABELS });
    expect(noMsg.lastTsMs).toBe(0);
    expect(noMsg.joinSeq).toBe(3);
  });
});

function group(overrides: Partial<GroupJson> = {}): GroupJson {
  return {
    groupId: "G-1",
    name: "项目组",
    owner: "self",
    members: ["self", "a"],
    rev: 1,
    state: "active",
    tsMs: 500,
    ...overrides,
  };
}

function groupMsg(overrides: Partial<GroupMessageJson> = {}): GroupMessageJson {
  return {
    id: "gm1",
    groupId: "G-1",
    senderId: "a",
    kind: "text",
    tsMs: 2000,
    text: "开会啦",
    media: null,
    status: "delivered",
    acks: [],
    ...overrides,
  };
}

describe("groupEntry 字段对齐（§2.2）", () => {
  it("subtitle 成员数；预览前缀发送者昵称；自己显「我」", () => {
    const e = groupEntry({
      group: group(),
      last: groupMsg(),
      unread: 1,
      selfPeerId: "self",
      membersLabel: "2 名成员",
      stateLabel: "",
      nickOf: (id) => (id === "a" ? "阿北" : id),
      joinSeq: 0,
      labels: LABELS,
    });
    expect(e.subtitle).toBe("2 名成员");
    expect(e.lastPreview).toBe("阿北：开会啦");
    expect(e.kindMark.groupBadge).toBe(true);
    const mine = groupEntry({
      group: group(),
      last: groupMsg({ senderId: "self", text: "收到" }),
      unread: 0,
      selfPeerId: "self",
      membersLabel: "",
      stateLabel: "",
      nickOf: (id) => id,
      joinSeq: 0,
      labels: LABELS,
    });
    expect(mine.lastPreview).toBe("我：收到");
    expect(mine.sendState).toBe("delivered");
  });

  it("非 active 状态徽标沿用映射；无消息 lastTsMs 回退群 tsMs", () => {
    const cases = [
      { state: "left", tone: "secondary" },
      { state: "kicked", tone: "destructive" },
      { state: "disbanded", tone: "outline" },
    ] as const;
    for (const { state, tone } of cases) {
      const e = groupEntry({
        group: group({ state }),
        last: null,
        unread: 0,
        selfPeerId: "self",
        membersLabel: "",
        stateLabel: state,
        nickOf: (id) => id,
        joinSeq: 0,
        labels: LABELS,
      });
      expect(e.statusBadge).toEqual({ tone, label: state });
      expect(e.lastTsMs).toBe(500);
    }
  });
});

describe("agentEntry 字段对齐（§2.2）", () => {
  const base = {
    endpointId: "E1",
    alias: "",
    wsUrl: "ws://127.0.0.1:8787",
    phase: "online" as const,
    connectFailed: false,
    connectionLabel: "在线",
    lastText: null as string | null,
    lastInteractionMs: 0,
    unread: 0,
    promptPending: false,
    joinSeq: 0,
    labels: LABELS,
  };

  it("别名缺省回退 host；Bot 图标 + 连接态色点", () => {
    const e = agentEntry(base);
    expect(e.title).toBe("127.0.0.1:8787");
    expect(e.kindMark.botIcon).toBe(true);
    expect(e.kindMark.dot).toBe("green");
    expect(e.host).toBe("127.0.0.1:8787");
  });

  it("连接中黄点/断开红点 + 断连徽标与 error sendState", () => {
    expect(agentEntry({ ...base, phase: "reconnecting" }).kindMark.dot).toBe("yellow");
    const off = agentEntry({ ...base, phase: "offline", connectFailed: true, connectionLabel: "离线" });
    expect(off.kindMark.dot).toBe("red");
    expect(off.statusBadge).toEqual({ tone: "destructive", label: "离线" });
    expect(off.sendState).toBe("error");
  });

  it("预览取最后文本截 40 字；无文本不回退拼接连接态（F07）", () => {
    const long = "y".repeat(AGENT_PREVIEW_MAX_CHARS + 5);
    const e = agentEntry({ ...base, lastText: long });
    expect(e.lastPreview).toBe("y".repeat(AGENT_PREVIEW_MAX_CHARS) + "…");
    // 连接态只由 subtitle/徽标单维度呈现，预览不重复同一文案
    expect(agentEntry(base).lastPreview).toBeNull();
  });

  it("回合进行中 sendState=pending", () => {
    expect(agentEntry({ ...base, promptPending: true }).sendState).toBe("pending");
  });
});

describe("混排排序与搜索过滤（§2.2/§2.4）", () => {
  const row = (over: Partial<ConversationEntry>): ConversationEntry => ({
    id: "x",
    kind: "friend",
    title: "t",
    subtitle: null,
    kindMark: { initial: "t", botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: null,
    lastTsMs: 0,
    unread: 0,
    sendState: null,
    host: null,
    joinSeq: 0,
    ...over,
  });

  it("lastTsMs 降序；无消息按加入序垫底", () => {
    const sorted = sortEntries([
      row({ id: "old", lastTsMs: 100 }),
      row({ id: "new", lastTsMs: 300 }),
      row({ id: "mid", lastTsMs: 200 }),
      row({ id: "join2", joinSeq: 2 }),
      row({ id: "join1", joinSeq: 1 }),
    ]);
    expect(sorted.map((e) => e.id)).toEqual(["new", "mid", "old", "join2", "join1"]);
  });

  it("标题/备注子串不分大小写；ID 前缀；agent host 子串", () => {
    const entries: ConversationEntry[] = [
      row({ id: "peerAAA", title: "小圆", subtitle: "同事" }),
      row({ id: "G-9", kind: "group", title: "项目组" }),
      row({ id: "E1", kind: "agent", title: "helper", host: "127.0.0.1:8787" }),
    ];
    expect(filterEntries(entries, "小圆").map((e) => e.id)).toEqual(["peerAAA"]);
    expect(filterEntries(entries, "同事").map((e) => e.id)).toEqual(["peerAAA"]);
    expect(filterEntries(entries, "peer").map((e) => e.id)).toEqual(["peerAAA"]);
    expect(filterEntries(entries, "G-").map((e) => e.id)).toEqual(["G-9"]);
    expect(filterEntries(entries, "8787").map((e) => e.id)).toEqual(["E1"]);
    expect(filterEntries(entries, "不存在")).toEqual([]);
  });
});

describe("formatUnreadCount（§2.3）", () => {
  it("≥100 显 99+", () => {
    expect(formatUnreadCount(99)).toBe("99");
    expect(formatUnreadCount(100)).toBe("99+");
    expect(formatUnreadCount(250)).toBe("99+");
  });
});

describe("wsHostOf", () => {
  it("解析 host，非法 URL 返回 null", () => {
    expect(wsHostOf("ws://127.0.0.1:8787")).toBe("127.0.0.1:8787");
    expect(wsHostOf("not-a-url")).toBeNull();
  });
});

describe("visibleGroups（已退群默认隐藏）", () => {
  const mk = (id: string, state: GroupJson["state"]): GroupJson => ({
    groupId: id, name: id, owner: "o", members: ["o"], rev: 1, state, tsMs: 500,
  });

  it("默认仅放行 active；开关打开全量显示；空列表安全", () => {
    const groups = [
      mk("G1", "active"),
      mk("G2", "left"),
      mk("G3", "kicked"),
      mk("G4", "disbanded"),
    ];
    expect(visibleGroups(groups, false).map((g) => g.groupId)).toEqual(["G1"]);
    expect(visibleGroups(groups, true).map((g) => g.groupId)).toEqual(["G1", "G2", "G3", "G4"]);
    expect(visibleGroups([], false)).toEqual([]);
  });
});
