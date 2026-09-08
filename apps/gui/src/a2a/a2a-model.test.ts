// a2a 数据面纯逻辑单测：卡帧分发（cards/push/error）、version 钳制、
// skills 归一（Q9 口径）与 slug 双写。通道/ store 的 IO 面由页测试矩阵覆盖。
import { describe, expect, it, vi } from "vitest";

import { cardKey, handleFrame, shouldReplace, type CardChannelCallbacks } from "./card-channel";
import { normalizeSkills, toSkillJson } from "./skills";
import type { AgentCardJson } from "./types";

function card(version: number, agentId = "a1"): AgentCardJson {
  return {
    agentId,
    name: "agent",
    description: "d",
    url: "a2a://peer/" + agentId,
    hostPeer: "peer",
    visibility: "public",
    ttlSecs: 300,
    version,
  };
}

const noopCb: CardChannelCallbacks = {
  onCards: vi.fn(),
  onRemoved: vi.fn(),
  onError: vi.fn(),
  onStatus: vi.fn(),
};

describe("handleFrame", () => {
  it("cards 帧：抽出 payload 并回调 onCards", () => {
    const onCards = vi.fn();
    handleFrame({ op: "cards", v: 1, id: 1, cards: [{ payload: card(1) }] }, { ...noopCb, onCards });
    expect(onCards).toHaveBeenCalledTimes(1);
    expect(onCards.mock.calls[0]![0][0].card.agentId).toBe("a1");
    expect(onCards.mock.calls[0]![0][0].issuedAtSecs).toBe(0);
  });

  it("push 帧：cards 与 removed 双面回调", () => {
    const onCards = vi.fn();
    const onRemoved = vi.fn();
    handleFrame(
      { op: "push", v: 1, id: 0, cards: [{ payload: card(2), issued_at: 42 }], removed: ["peer/a2"] },
      { ...noopCb, onCards, onRemoved },
    );
    expect(onCards.mock.calls[0]![0][0].issuedAtSecs).toBe(42);
    expect(onRemoved).toHaveBeenCalledWith(["peer/a2"]);
  });

  it("error 帧：原文上抛不静默", () => {
    const onError = vi.fn();
    handleFrame({ op: "error", v: 1, id: 9, code: "denied", message: "no" }, { ...noopCb, onError });
    expect(onError).toHaveBeenCalledWith("denied: no");
  });

  it("坏形载荷（缺 agentId/hostPeer）被丢弃不崩", () => {
    const onCards = vi.fn();
    handleFrame({ op: "cards", v: 1, id: 1, cards: [{ payload: {} }, "junk"] }, { ...noopCb, onCards });
    expect(onCards).not.toHaveBeenCalled();
  });
});

describe("version 钳制与键", () => {
  it("同键 version 升序才覆盖", () => {
    expect(shouldReplace(undefined, card(1))).toBe(true);
    expect(shouldReplace(card(2), card(3))).toBe(true);
    expect(shouldReplace(card(3), card(2))).toBe(false);
    expect(shouldReplace(card(3), card(3))).toBe(false);
  });
  it("cardKey = hostPeer/agentId（与宿主 removed 键同形）", () => {
    expect(cardKey(card(1))).toBe("peer/a1");
  });
});

describe("normalizeSkills（拍板 Q9）", () => {
  it("trim + 去空 + 去重 + 截断 10 条", () => {
    const { skills, dropped } = normalizeSkills([" a ", "a", "", "b"]);
    expect(skills).toEqual(["a", "b"]);
    expect(dropped).toBe(0);
    const many = normalizeSkills(Array.from({ length: 12 }, (_, i) => "s" + i));
    expect(many.skills).toHaveLength(10);
    expect(many.dropped).toBe(2);
  });
});

describe("toSkillJson", () => {
  it("中文 chip 用稳定 hash 兜底 id，name 保留原文，同输入同 id", () => {
    const first = toSkillJson("代码审查")!;
    expect(first.name).toBe("代码审查");
    expect(first.id).toMatch(/^skill-[0-9a-f]+$/);
    expect(toSkillJson("代码审查")!.id).toBe(first.id);
  });
  it("纯符号 chip 无 slug 也走 hash 兜底（不静默丢卡）", () => {
    const skill = toSkillJson("///")!;
    expect(skill.id).toMatch(/^skill-[0-9a-f]+$/);
    expect(skill.name).toBe("///");
  });
  it("空串 chip 拒绝", () => {
    expect(toSkillJson("   ")).toBeNull();
  });
  it("英文 chip 小写连字符化", () => {
    expect(toSkillJson("Code Review")).toEqual({ id: "code-review", name: "Code Review" });
  });
});
