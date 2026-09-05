// transcript 纯模型单测：气泡聚合、思考折叠、结算与未知更新计数。
import { describe, expect, it } from "vitest";

import {
  applyUpdate,
  applyUserPrompt,
  emptyTranscript,
  settleTranscript,
  toolIoView,
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
    st = applyUpdate(st, { sessionUpdate: "future_kind" });
    expect(st.turns).toHaveLength(0);
    expect(st.ignoredUpdates).toBe(1);
  });
});

describe("tool timeline", () => {
  const call = {
    sessionUpdate: "tool_call" as const,
    toolCallId: "call-1",
    title: "Reading config",
    kind: "read",
    status: "pending" as const,
    rawInput: { path: "a.ts" },
  };

  it("tool_call 建时间线节点；update 同 id 原地迁移状态，字段缺省保持", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, call);
    expect(st.turns[0]).toMatchObject({
      kind: "tool",
      toolCallId: "call-1",
      title: "Reading config",
      toolKind: "read",
      status: "pending",
      inputText: '{"path":"a.ts"}',
      outputText: "",
    });
    st = applyUpdate(st, { sessionUpdate: "tool_call_update", toolCallId: "call-1", status: "in_progress" });
    expect(st.turns).toHaveLength(1);
    expect(st.turns[0]).toMatchObject({ status: "in_progress", title: "Reading config" });
    st = applyUpdate(st, {
      sessionUpdate: "tool_call_update",
      toolCallId: "call-1",
      status: "completed",
      content: [{ type: "content", content: { type: "text", text: "3 files found" } }],
    });
    expect(st.turns[0]).toMatchObject({ status: "completed", outputText: "3 files found" });
  });

  it("failed 迁移与未知 id 的 update 新建节点（title 回退 toolCallId）", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, call);
    st = applyUpdate(st, { sessionUpdate: "tool_call_update", toolCallId: "call-2", status: "failed" });
    expect(st.turns).toHaveLength(2);
    expect(st.turns[1]).toMatchObject({ toolCallId: "call-2", title: "call-2", status: "failed" });
  });

  it("工具节点不打断同气泡聚合（消息块仍归并到 open assistant）", () => {
    let st = emptyTranscript();
    st = applyUpdate(st, msgChunk("部"));
    st = applyUpdate(st, call);
    st = applyUpdate(st, msgChunk("分A"));
    st = applyUpdate(st, msgChunk("分B"));
    expect(st.turns.map((t) => t.kind)).toEqual(["assistant", "tool"]);
    expect(st.turns[0]).toMatchObject({ text: "部分A分B", streaming: true });
  });
});

describe("tool io fold", () => {
  it("短文本不折叠，preview 即原文", () => {
    expect(toolIoView("line-1")).toEqual({ collapsible: false, preview: "line-1" });
    expect(toolIoView("a\nb\nc\nd\ne\nf")).toEqual({ collapsible: false, preview: "a\nb\nc\nd\ne\nf" });
  });

  it("超过 6 行折叠，preview 截到前 6 行", () => {
    const text = Array.from({ length: 10 }, (_, i) => "line-" + i).join("\n");
    const view = toolIoView(text);
    expect(view.collapsible).toBe(true);
    expect(view.preview).toBe("line-0\nline-1\nline-2\nline-3\nline-4\nline-5");
  });

  it("超长单行折叠，preview 截到 400 字符", () => {
    const view = toolIoView("x".repeat(401));
    expect(view.collapsible).toBe(true);
    expect(view.preview.length).toBe(400);
  });
});
