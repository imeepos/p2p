// 可脚本化的本地 WS mock：dev（VITE_MOCK_IPC=1）与测试共用同一实现，
// 行为对齐 apps/acp-console/README.md（token 错误 4403、未知 peer 4500、
// agent 断流 1000）。会话状态存于单例 console，跨重连存活供侧栏断言。
import type { WsLike, WebSocketFactory } from "./ws-factory";
import type {
  InitializeResult,
  SessionSummary,
  SessionUpdate,
  SessionUpdateParams,
} from "./protocol";

export type MockPromptStep =
  | { kind: "thought"; text: string }
  | { kind: "message"; text: string }
  | { kind: "stop"; reason: string };

export interface MockConsoleConfig {
  token: string;
  peers: string[];
  promptScript: MockPromptStep[];
  openDelayMs: number;
  chunkDelayMs: number;
}

const DEFAULT_CONFIG: MockConsoleConfig = {
  token: "mock-token",
  peers: ["mock-peer"],
  promptScript: [
    { kind: "thought", text: "thinking through the request" },
    { kind: "message", text: "Hello from the mock agent." },
    { kind: "stop", reason: "end_turn" },
  ],
  openDelayMs: 10,
  chunkDelayMs: 10,
};

interface WireMessage {
  jsonrpc?: string;
  id?: number;
  method?: string;
  params?: Record<string, unknown>;
  result?: unknown;
  error?: { code: number; message: string };
}

const MOCK_AGENT_INFO: InitializeResult = {
  protocolVersion: 1,
  agentInfo: { name: "mock-agent", version: "0.1.0" },
  agentCapabilities: {
    loadSession: true,
    promptCapabilities: { embeddedContext: false },
  },
};

class MockAcpConsole {
  config: MockConsoleConfig = { ...DEFAULT_CONFIG };
  sessions = new Map<string, SessionSummary>();
  live: MockSocket[] = [];
  private sessionSeq = 0;

  configure(patch: Partial<MockConsoleConfig>): void {
    this.config = { ...this.config, ...patch };
  }

  reset(): void {
    this.config = { ...DEFAULT_CONFIG };
    this.sessions.clear();
    this.live = [];
    this.sessionSeq = 0;
  }

  nextSessionId(): string {
    this.sessionSeq += 1;
    return "s-" + String(this.sessionSeq).padStart(3, "0");
  }

  broadcast(method: string, params: unknown): void {
    for (const socket of this.live) socket.serverPush(method, params);
  }

  dropAll(code = 1000, reason = "agent-stream-dropped"): void {
    for (const socket of [...this.live]) socket.serverClose(code, reason);
  }

  remove(socket: MockSocket): void {
    this.live = this.live.filter((s) => s !== socket);
  }
}

export const mockAcpConsole = new MockAcpConsole();

export class MockSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  private closed = false;
  private promptTimer: number | null = null;
  private pendingPrompt: { id: number; sessionId: string } | null = null;

  constructor(private readonly url: string, private readonly box: MockAcpConsole) {
    window.setTimeout(() => this.open(), box.config.openDelayMs);
  }

  private open(): void {
    const params = new URL(this.url).searchParams;
    if (params.get("token") !== this.box.config.token) {
      this.fireClose(4403, "denied:bad-token");
      return;
    }
    const peer = params.get("peer");
    if (!peer || !this.box.config.peers.includes(peer)) {
      this.fireClose(4500, "dial-failed");
      return;
    }
    if (this.closed) return;
    this.box.live.push(this);
    this.onopen?.();
  }

  send(data: string): void {
    if (this.closed) {
      console.warn("[mock-acp] 已关闭的 socket 收到帧，已丢弃");
      return;
    }
    for (const line of data.split("\n")) {
      if (line.trim().length > 0) this.handle(JSON.parse(line) as WireMessage);
    }
  }

  close(code = 1000, reason = "client-close"): void {
    this.serverClose(code, reason);
  }

  /** console/agent 侧主动关断（区别于客户端 close 的对称入口） */
  serverClose(code: number, reason: string): void {
    if (this.closed) return;
    this.cancelPromptPlayback();
    this.fireClose(code, reason);
  }

  serverPush(method: string, params: unknown): void {
    if (this.closed) return;
    this.deliver({ jsonrpc: "2.0", method, params: params as Record<string, unknown> });
  }

  private fireClose(code: number, reason: string): void {
    this.closed = true;
    this.box.remove(this);
    this.onclose?.({ code, reason });
  }

  private deliver(msg: WireMessage): void {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }

  private reply(id: number, result: unknown): void {
    this.deliver({ jsonrpc: "2.0", id, result });
  }

  private replyError(id: number, code: number, message: string): void {
    this.deliver({ jsonrpc: "2.0", id, error: { code, message } });
  }

  private handle(msg: WireMessage): void {
    if (typeof msg.method !== "string") return;
    // 通知面（无 id）：session/cancel 即时结算进行中的 prompt
    if (msg.method === "session/cancel") {
      this.settlePrompt("cancelled");
      return;
    }
    if (typeof msg.id !== "number") return;
    const params = msg.params ?? {};
    switch (msg.method) {
      case "initialize":
        this.reply(msg.id, MOCK_AGENT_INFO);
        return;
      case "session/new":
        this.handleNew(msg.id);
        return;
      case "session/prompt":
        this.handlePrompt(msg.id, params);
        return;
      case "session/list":
        this.reply(msg.id, { sessions: [...this.box.sessions.values()] });
        return;
      case "session/resume":
        this.handleResume(msg.id, params);
        return;
      case "session/close":
        this.handleCloseSession(msg.id, params);
        return;
      default:
        this.replyError(msg.id, -32601, "method not found: " + msg.method);
    }
  }

  private handleNew(id: number): void {
    const sessionId = this.box.nextSessionId();
    this.box.sessions.set(sessionId, { sessionId, title: "session " + sessionId });
    this.reply(id, { sessionId });
  }

  private handleResume(id: number, params: Record<string, unknown>): void {
    const sessionId = String(params.sessionId ?? "");
    if (!this.box.sessions.has(sessionId)) {
      this.replyError(id, -32002, "session not found");
      return;
    }
    this.reply(id, { sessionId });
  }

  private handleCloseSession(id: number, params: Record<string, unknown>): void {
    const sessionId = String(params.sessionId ?? "");
    this.box.sessions.delete(sessionId);
    this.reply(id, {});
  }

  private handlePrompt(id: number, params: Record<string, unknown>): void {
    const sessionId = String(params.sessionId ?? "");
    if (!this.box.sessions.has(sessionId)) {
      this.replyError(id, -32002, "session not found");
      return;
    }
    if (this.pendingPrompt) {
      this.replyError(id, -32001, "prompt already running");
      return;
    }
    this.pendingPrompt = { id, sessionId };
    this.playScript(this.box.config.promptScript, 0);
  }

  private playScript(steps: MockPromptStep[], index: number): void {
    if (this.closed) return;
    if (!this.pendingPrompt || index >= steps.length) {
      this.settlePrompt("end_turn");
      return;
    }
    const step = steps[index];
    this.promptTimer = window.setTimeout(() => {
      this.applyStep(step);
      this.playScript(steps, index + 1);
    }, this.box.config.chunkDelayMs);
  }

  private applyStep(step: MockPromptStep): void {
    const pending = this.pendingPrompt;
    if (!pending) return;
    if (step.kind === "stop") {
      this.settlePrompt(step.reason);
      return;
    }
    const update: SessionUpdate =
      step.kind === "thought"
        ? { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: step.text } }
        : { sessionUpdate: "agent_message_chunk", content: { type: "text", text: step.text } };
    const params: SessionUpdateParams = { sessionId: pending.sessionId, update };
    this.box.broadcast("session/update", params);
  }

  private settlePrompt(reason: string): void {
    const pending = this.pendingPrompt;
    this.cancelPromptPlayback();
    if (pending) this.reply(pending.id, { stopReason: reason });
  }

  private cancelPromptPlayback(): void {
    if (this.promptTimer !== null) {
      window.clearTimeout(this.promptTimer);
      this.promptTimer = null;
    }
    this.pendingPrompt = null;
  }
}

export function createMockWsFactory(): WebSocketFactory {
  return (url) => new MockSocket(url, mockAcpConsole);
}
