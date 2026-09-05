// transcript 轮次模型：session/update 块流到聊天气泡/思考面板/工具时间线的纯归并逻辑。
// assistant 气泡只在流式期间聚合块；prompt 结算后新块开新气泡（对齐 ACP
// 每 prompt 一轮的语义）。工具轮按 toolCallId 原地迁移状态（ACP tool_call/update）。
// 未知 update 种类计数留痕，不静默丢弃。
import type { SessionUpdate, ToolCallPayload, ToolCallStatus } from "./protocol";

export type Turn =
  | { kind: "user"; id: number; text: string }
  | {
      kind: "assistant";
      id: number;
      text: string;
      streaming: boolean;
      stopReason: string | null;
    }
  | { kind: "thought"; id: number; text: string; open: boolean }
  | {
      kind: "tool";
      id: number;
      toolCallId: string;
      title: string;
      toolKind: string | null;
      status: ToolCallStatus;
      inputText: string;
      outputText: string;
    };

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
  if (kind === "agent_message_chunk") return appendChunk(state, "message", chunkText(update));
  if (kind === "agent_thought_chunk") return appendChunk(state, "thought", chunkText(update));
  if (kind === "tool_call" || kind === "tool_call_update") {
    return upsertToolCall(state, update as unknown as ToolCallPayload);
  }
  const next = clone(state);
  next.ignoredUpdates += 1;
  return next;
}

/** 工具轮原地上迁：同 toolCallId 合并字段，未提供的字段保持原值 */
function upsertToolCall(state: TranscriptState, tool: ToolCallPayload): TranscriptState {
  const next = clone(state);
  const existing = next.turns.find(
    (t): t is Extract<Turn, { kind: "tool" }> =>
    t.kind === "tool" && t.toolCallId === tool.toolCallId,
  );
  if (!existing) {
    next.turns.push({
      kind: "tool",
      id: next.nextId,
      toolCallId: tool.toolCallId,
      title: tool.title ?? tool.toolCallId,
      toolKind: tool.kind ?? null,
      status: tool.status ?? "pending",
      inputText: ioText(tool.rawInput),
      outputText: outputText(tool),
    });
    next.nextId += 1;
    return next;
  }
  if (tool.title !== undefined) existing.title = tool.title;
  if (tool.kind !== undefined) existing.toolKind = tool.kind;
  if (tool.status !== undefined) existing.status = tool.status;
  if (tool.rawInput !== undefined) existing.inputText = ioText(tool.rawInput);
  const out = outputText(tool);
  if (out !== "") existing.outputText = out;
  return next;
}

/** rawInput/rawOutput 是任意 JSON：字符串原样，其余序列化，序列化失败留 observable 信号 */
function ioText(raw: unknown): string {
  if (raw === undefined || raw === null) return "";
  if (typeof raw === "string") return raw;
  try {
    return JSON.stringify(raw);
  } catch {
    console.warn("[acp] tool_call rawInput/rawOutput 序列化失败，降级 String()");
    return String(raw);
  }
}

/** 结果文本：优先 content 文本块（{type:"content",content:{type:"text",...}}），缺省 rawOutput */
function outputText(tool: ToolCallPayload): string {
  const fromContent = contentText(tool.content);
  if (fromContent !== "") return fromContent;
  return ioText(tool.rawOutput);
}

function contentText(content: unknown): string {
  if (!Array.isArray(content)) return "";
  const parts: string[] = [];
  for (const item of content) {
    const inner = (item as { content?: { text?: unknown } }).content;
    if (inner && typeof inner.text === "string") parts.push(inner.text);
  }
  return parts.join("\n");
}

function chunkText(update: SessionUpdate): string {
  const content = (update as { content?: { text?: unknown } }).content;
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

/** 错误结算约定值：气泡失败徽章据此渲染（transcript.tsx isErrorStop） */
export const ERROR_STOP_REASON = "error";

export function settleTranscript(
  state: TranscriptState,
  stopReason: string | null,
): TranscriptState {
  const next = clone(state);
  const open = next.openAssistantId;
  next.openAssistantId = null;
  next.openThoughtId = null;
  if (open !== null) {
    const turn = next.turns.find((t) => t.id === open);
    if (turn && turn.kind === "assistant") {
      turn.streaming = false;
      turn.stopReason = stopReason;
      return next;
    }
  }
  // 失败结算必须有落点：无流式气泡（如发后即失败）补错误占位轮承载徽章
  if (stopReason === ERROR_STOP_REASON) {
    next.turns.push({
      kind: "assistant",
      id: next.nextId,
      text: "",
      streaming: false,
      stopReason: ERROR_STOP_REASON,
    });
    next.nextId += 1;
  }
  return next;
}

export function toggleThought(state: TranscriptState, id: number): TranscriptState {
  const next = clone(state);
  const turn = next.turns.find((t) => t.id === id);
  if (turn && turn.kind === "thought") turn.open = !turn.open;
  return next;
}

/** 工具入参/结果折叠阈值：超过约 6 行（或超长单行）默认收起，可展开 */
export const TOOL_IO_COLLAPSE_LINES = 6;

const TOOL_IO_MAX_CHARS = 400;

export interface ToolIoView {
  /** 内容超阈值需要折叠；短文本不渲染展开开关 */
  collapsible: boolean;
  /** 折叠态预览：前 6 行且不超过 400 字符 */
  preview: string;
}

export function toolIoView(text: string): ToolIoView {
  const collapsible =
    text.split("\n").length > TOOL_IO_COLLAPSE_LINES || text.length > TOOL_IO_MAX_CHARS;
  if (!collapsible) return { collapsible: false, preview: text };
  let preview = text.split("\n", TOOL_IO_COLLAPSE_LINES).join("\n");
  if (preview.length > TOOL_IO_MAX_CHARS) preview = preview.slice(0, TOOL_IO_MAX_CHARS);
  return { collapsible: true, preview };
}