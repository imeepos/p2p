// 可脚本化的本地 WS mock：dev（VITE_MOCK_IPC=1）与测试共用同一实现。
// 行为按 2026-09-05 真机对拍实测对齐 acp-console（docs/notes/
// 2026-09-05-acp-real-calibration.md）：错 token = HTTP 401 升级拒绝（客户端
// 视角 error + 1006 空 reason）、未知 peer = 先 open 后 Close(4500)、agent 桥
// 拒绝握手 = Close(4403, "denied:<code>")、对端死亡 = 无 Close 帧 1006；
// 下行帧为二进制（Uint8Array），可多行合帧。会话状态存单例，跨重连存活。
import { MockPromptPlayer, type MockPromptStep } from "./mock-acp-prompt";
import type { InitializeResult, SessionSummary } from "./protocol";
import type { WsLike, WebSocketFactory } from "./ws-factory";

export interface MockConsoleConfig {
  token: string;
  /** 可拨通且 agent 侧放行的 peer */
  peers: string[];
  /** 可拨通但 agent 桥拒绝握手（4403 denied）的 peer */
  deniedPeers: string[];
  promptScript: MockPromptStep[];
  openDelayMs: number;
  chunkDelayMs: number;
}

const DEFAULT_CONFIG: MockConsoleConfig = {
  token: "mock-token",
  peers: ["mock-peer"],
  deniedPeers: [],
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
    this.config = { ...DEFAULT_CONFIG, deniedPeers: [] };
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

  /** 对端断流：真机实测 agent 死亡后 console 不发 Close 帧，客户端见 1006 空 reason
   *  （优雅 EOF 路径才有 1000 "peer closed"，用 serverClose(1000, "peer closed") 显式模拟） */
  dropAll(code = 1006, reason = ""): void {
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
  private player = new MockPromptPlayer(
    (method, params) => this.box.broadcast(method, params),
    (id, result) => this.reply(id, result),
    () => this.box.config.chunkDelayMs,
  );

  constructor(private readonly url: string, private readonly box: MockAcpConsole) {
    window.setTimeout(() => this.open(), box.config.openDelayMs);
  }

  private open(): void {
    const params = new URL(this.url).searchParams;
    // 真机对拍 R3a：console 在 HTTP 升级层 401 拒绝，客户端只见 error + 1006 空 reason
    if (params.get("token") !== this.box.config.token) {
      this.fireAuthRejected();
      return;
    }
    if (this.closed) return;
    // 真机对拍 R3b：WS 先 accept（onopen 先行），拨号/握手结果以 Close 帧异步到达
    this.box.live.push(this);
    this.onopen?.();
    const peer = params.get("peer");
    if (peer && this.box.config.deniedPeers.includes(peer)) {
      this.fireClose(4403, "denied:peer-not-allowed");
      return;
    }
    if (!peer || !this.box.config.peers.includes(peer)) {
      this.fireClose(4500, "dial-failed");
    }
  }

  private fireAuthRejected(): void {
    this.onerror?.({ message: "unexpected server response (401)" });
    this.fireClose(1006, "");
  }

  send(data: string): void {
    if (this.closed) {
      console.warn("[mock-acp] 已关闭的 socket 收到帧，已丢弃");
      return;
    }
    for (const line of data.split("\n")) {
      if (line.trim().length === 0) continue;
      try {
        this.handle(JSON.parse(line) as WireMessage);
      } catch (error) {
        console.warn("[mock-acp] 非 JSON 行，已丢弃", error);
      }
    }
  }

  close(code = 1000, reason = "client-close"): void {
    this.serverClose(code, reason);
  }

  /** console/agent 侧主动关断（区别于客户端 close 的对称入口） */
  serverClose(code: number, reason: string): void {
    if (this.closed) return;
    this.player.stop();
    this.fireClose(code, reason);
  }

  serverPush(method: string, params: unknown): void {
    if (this.closed) return;
    this.deliver({ jsonrpc: "2.0", method, params: params as Record<string, unknown> });
  }

  /** 模拟 console 64KiB 块合帧：多条通知并入一个二进制帧（真机对拍 R3i 实测） */
  serverPushCoalesced(notifications: Array<{ method: string; params: unknown }>): void {
    if (this.closed) return;
    const text = notifications
      .map((n) => JSON.stringify({ jsonrpc: "2.0", method: n.method, params: n.params }))
      .join("\n");
    this.onmessage?.({ data: new TextEncoder().encode(text + "\n") });
  }

  private fireClose(code: number, reason: string): void {
    this.closed = true;
    this.box.remove(this);
    this.onclose?.({ code, reason });
  }

  private deliver(msg: WireMessage): void {
    // console 泵只发 Binary 帧（真机对拍 R3i），mock 同形输出 Uint8Array；
    // ndjson 行界必须带行尾换行，否则 GUI 侧行重组器会当残行挂起
    this.onmessage?.({ data: new TextEncoder().encode(JSON.stringify(msg) + "\n") });
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
      this.player.cancel("cancelled");
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
      case "session/delete":
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
    if (this.player.busy) {
      this.replyError(id, -32001, "prompt already running");
      return;
    }
    this.player.start(id, sessionId, this.box.config.promptScript);
  }
}

export function createMockWsFactory(): WebSocketFactory {
  return (url) => new MockSocket(url, mockAcpConsole);
}
