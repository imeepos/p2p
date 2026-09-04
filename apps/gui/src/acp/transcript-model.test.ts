// transcript 纯模型单测：气泡聚合、思考折叠、结算与未知更新计数。
import { describe, expect, it } from "vitest";

import {
  applyUpdate,
  applyUserPrompt,
  emptyTranscript,
  settleTranscript,
  toggleThought,
} from "./transcript-model";

function msgChunk(text: string) {
  return { sessionUpdate: "agent_message_chunk", content: { type: "text", text } };
}

function thoughtChunk(text: string) {
  return { sessionUpdate: "agent_thought_chunk", content: { type: "text", text } };
}

describe("transcript model", () => {
  it("消息块聚合同一流式气泡，结算后新块开新气泡", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, msgChunk("你好"));
    st = applyUpdate(st, msgChunk("，世界"));
    expect(st.turns).toHaveLength(1);
    const bubble = st.turns[0];
    expect(bubble.kind).toBe("assistant");
    if (bubble.kind === "assistant") {
      expect(bubble.text).toBe("你好，世界");
      expect(bubble.streaming).toBe(true);
    }
    st = settleTranscript(st, "end_turn");
    expect(st.turns[0]).toMatchObject({ streaming: false, stopReason: "end_turn" });
    st = applyUpdate(st, msgChunk("新一轮"));
    expect(st.turns).toHaveLength(2);
  });

  it("思考块归并为折叠面板，toggle 翻转 open", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, thoughtChunk("第一步"));
    st = applyUpdate(st, thoughtChunk("+第二步"));
    expect(st.turns).toHaveLength(1);
    const turn = st.turns[0];
    expect(turn.kind).toBe("thought");
    if (turn.kind === "thought") {
      expect(turn.text).toBe("第一步+第二步");
      expect(turn.open).toBe(false);
      st = toggleThought(st, turn.id);
      expect(st.turns[0]).toMatchObject({ open: true });
    }
  });

  it("user prompt 开新轮并重置聚合指针", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, msgChunk("旧气泡"));
    st = settleTranscript(st, null);
    st = applyUserPrompt(st, "提问");
    expect(st.turns[1]).toMatchObject({ kind: "user", text: "提问" });
    st = applyUpdate(st, msgChunk("新回答"));
    expect(st.turns).toHaveLength(3);
    expect(st.turns[2]).toMatchObject({ kind: "assistant", text: "新回答", streaming: true });
  });

  it("未知 update 种类计数留痕不静默丢弃", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, { sessionUpdate: "tool_call", toolCall: {} });
    expect(st.turns).toHaveLength(0);
    expect(st.ignoredUpdates).toBe(1);
  });
});
