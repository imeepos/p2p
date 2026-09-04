// 可脚本化 prompt 回放器：把 MockPromptStep 序列按 chunkDelayMs 播成
// session/update 流，终态以 stopReason 结算 prompt 请求（mock agent 语义面）。
import type { SessionUpdate, SessionUpdateParams } from "./protocol";

export type MockPromptStep =
  | { kind: "thought"; text: string }
  | { kind: "message"; text: string }
  | { kind: "stop"; reason: string };

export class MockPromptPlayer {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private pending: { id: number; sessionId: string } | null = null;

  constructor(
    private readonly broadcast: (method: string, params: unknown) => void,
    private readonly reply: (id: number, result: unknown) => void,
    private readonly delayMs: () => number,
  ) {}

  get busy(): boolean {
    return this.pending !== null;
  }

  start(id: number, sessionId: string, steps: MockPromptStep[]): void {
    this.pending = { id, sessionId };
    this.play(steps, 0);
  }

  /** session/cancel 通知：结算进行中的 prompt 为指定 stopReason */
  cancel(reason = "cancelled"): void {
    const pending = this.pending;
    this.stop();
    if (pending) this.reply(pending.id, { stopReason: reason });
  }

  /** 断流路径：静默丢弃 pending（不回包，由连接层以关断结算） */
  stop(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.pending = null;
  }

  private play(steps: MockPromptStep[], index: number): void {
    if (!this.pending || index >= steps.length) {
      this.cancel("end_turn");
      return;
    }
    const step = steps[index];
    this.timer = setTimeout(() => {
      this.apply(step);
      this.play(steps, index + 1);
    }, this.delayMs());
  }

  private apply(step: MockPromptStep): void {
    const pending = this.pending;
    if (!pending) return;
    if (step.kind === "stop") {
      this.cancel(step.reason);
      return;
    }
    const update: SessionUpdate =
      step.kind === "thought"
        ? { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: step.text } }
        : { sessionUpdate: "agent_message_chunk", content: { type: "text", text: step.text } };
    const params: SessionUpdateParams = { sessionId: pending.sessionId, update };
    this.broadcast("session/update", params);
  }
}
