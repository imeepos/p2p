// transcript 轮次模型：session/update 块流到聊天气泡/思考面板的纯归并逻辑。
// assistant 气泡只在流式期间聚合块；prompt 结算后新块开新气泡（对齐 ACP
// 每 prompt 一轮的语义）。未知 update 种类计数留痕，不静默丢弃。
import type { SessionUpdate } from "./protocol";

export type Turn =
  | { kind: "user"; id: number; text: string }
  | {
      kind: "assistant";
      id: number;
      text: string;
      streaming: boolean;
      stopReason: string | null;
    }
  | { kind: "thought"; id: number; text: string; open: boolean };

export interface TranscriptState {
  turns: Turn[];
  nextId: number;
  openAssistantId: number | null;
  openThoughtId: number | null;
  ignoredUpdates: number;
}

export function emptyTranscript(): TranscriptState {
  return {
    turns: [],
    nextId: 1,
    openAssistantId: null,
    openThoughtId: null,
    ignoredUpdates: 0,
  };
}

function clone(state: TranscriptState): TranscriptState {
  return { ...state, turns: state.turns.map((turn) => ({ ...turn })) };
}

export function applyUserPrompt(state: TranscriptState, text: string): TranscriptState {
  const next = clone(state);
  next.turns.push({ kind: "user", id: next.nextId, text });
  next.nextId += 1;
  next.openAssistantId = null;
  next.openThoughtId = null;
  return next;
}

export function applyUpdate(state: TranscriptState, update: SessionUpdate): TranscriptState {
  const kind = update.sessionUpdate;
  const text = chunkText(update);
  if (kind === "agent_message_chunk") return appendChunk(state, "message", text);
  if (kind === "agent_thought_chunk") return appendChunk(state, "thought", text);
  const next = clone(state);
  next.ignoredUpdates += 1;
  return next;
}

function chunkText(update: SessionUpdate): string {
  const content = update.content as { text?: unknown } | undefined;
  if (content && typeof content.text === "string") return content.text;
  return "";
}

function appendChunk(
  state: TranscriptState,
  target: "message" | "thought",
  text: string,
): TranscriptState {
  const next = clone(state);
  const openId = target === "message" ? next.openAssistantId : next.openThoughtId;
  const existing = openId === null ? undefined : next.turns.find((t) => t.id === openId);
  if (existing && existing.kind === "assistant" && target === "message") {
    existing.text += text;
    return next;
  }
  if (existing && existing.kind === "thought" && target === "thought") {
    existing.text += text;
    return next;
  }
  if (target === "message") {
    next.turns.push({
      kind: "assistant",
      id: next.nextId,
      text,
      streaming: true,
      stopReason: null,
    });
    next.openAssistantId = next.nextId;
  } else {
    next.turns.push({ kind: "thought", id: next.nextId, text, open: false });
    next.openThoughtId = next.nextId;
  }
  next.nextId += 1;
  return next;
}

export function settleTranscript(
  state: TranscriptState,
  stopReason: string | null,
): TranscriptState {
  const next = clone(state);
  if (next.openAssistantId !== null) {
    const turn = next.turns.find((t) => t.id === next.openAssistantId);
    if (turn && turn.kind === "assistant") {
      turn.streaming = false;
      turn.stopReason = stopReason;
    }
  }
  next.openAssistantId = null;
  next.openThoughtId = null;
  return next;
}

export function toggleThought(state: TranscriptState, id: number): TranscriptState {
  const next = clone(state);
  const turn = next.turns.find((t) => t.id === id);
  if (turn && turn.kind === "thought") turn.open = !turn.open;
  return next;
}
