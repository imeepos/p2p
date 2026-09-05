import type { TranscriptState } from "./transcript-model";

// §2.2 agent 条目摘要：transcript 最后一条 user/agent 文本（截断归
// conversation-entry.agentEntry，这里只取原文）；无会话/无文本返回 null。

export function lastTurnText(
  transcripts: Record<string, TranscriptState>,
  sessionId: string | null,
): string | null {
  if (!sessionId) return null;
  const turns = transcripts[sessionId]?.turns ?? [];
  for (let i = turns.length - 1; i >= 0; i -= 1) {
    const turn = turns[i];
    if (turn && (turn.kind === "user" || turn.kind === "assistant") && turn.text.trim()) {
      return turn.text;
    }
  }
  return null;
}
