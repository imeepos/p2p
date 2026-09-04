// ACP 客户端薄层：JSON-RPC over 本地 WS 的最小封装。
// console 是纯字节泵，本层即 GUI 的 ACP 协议智能所在；断线自动重连并在
// 事件面给出可观测提示（阶段迁移 + 关闭码分类），不做静默重试。
import {
  buildWsUrl,
  classifyClose,
  initializeParams,
  promptParams,
  type AcpCloseInfo,
  type AcpEndpoint,
  type AcpPhase,
  type InitializeResult,
  type PromptResult,
  type SessionListResult,
  type SessionNewResult,
  type SessionSummary,
} from "./protocol";
import type { WsLike, WebSocketFactory } from "./ws-factory";

const REQUEST_TIMEOUT_MS = 30_000;
const BASE_RECONNECT_DELAY_MS = 1_000;
const MAX_RECONNECT_DELAY_MS = 15_000;

export interface ReconnectPolicy {
  maxAttempts: number;
  baseDelayMs: number;
  maxDelayMs: number;
}

export const DEFAULT_RECONNECT: ReconnectPolicy = {
  maxAttempts: 3,
  baseDelayMs: BASE_RECONNECT_DELAY_MS,
  maxDelayMs: MAX_RECONNECT_DELAY_MS,
};

export interface AcpConnectionEvents {
  onPhase(phase: AcpPhase): void;
  onNotification(method: string, params: unknown): void;
  onCloseInfo(info: AcpCloseInfo): void;
  onReconnect(attempt: number, maxAttempts: number): void;
}

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: number;
}

interface JsonRpcWire {
  jsonrpc?: string;
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code: number; message: string };
}

export class AcpConnection {
  private ws: WsLike | null = null;
  private nextId = 1;
  private pending = new Map<number, PendingRequest>();
  private reconnectTimer: number | null = null;
  private attempts = 0;
  private userClosed = false;

  constructor(
    private readonly endpoint: AcpEndpoint,
    private readonly factory: WebSocketFactory,
    private readonly events: AcpConnectionEvents,
    private readonly policy: ReconnectPolicy = DEFAULT_RECONNECT,
  ) {}

  connect(): void {
    this.userClosed = false;
    this.clearReconnectTimer();
    this.events.onPhase("connecting");
    const ws = this.factory(buildWsUrl(this.endpoint));
    this.ws = ws;
    ws.onopen = () => {
      this.attempts = 0;
      this.events.onPhase("online");
    };
    ws.onmessage = (ev) => this.dispatch(String(ev.data));
    ws.onerror = (ev) => {
      console.warn("[acp] WS 错误信号", ev.message);
    };
    ws.onclose = (ev) => this.handleClose(ev.code, ev.reason);
  }

  private handleClose(code: number, reason: string): void {
    this.ws = null;
    for (const [, p] of this.pending) {
      window.clearTimeout(p.timer);
      p.reject(new Error("acp-connection-closed"));
    }
    this.pending.clear();
    const info = classifyClose(code, reason);
    this.events.onCloseInfo(info);
    if (this.userClosed || info.kind === "denied" || info.kind === "dial-failed") {
      // 鉴权/拨号失败是配置错误：重连必然同败，转 offline 留给用户改参数
      this.events.onPhase("offline");
      return;
    }
    if (this.attempts >= this.policy.maxAttempts) {
      console.warn("[acp] 重连次数用尽，转 offline", this.attempts);
      this.events.onPhase("offline");
      return;
    }
    this.attempts += 1;
    this.events.onReconnect(this.attempts, this.policy.maxAttempts);
    this.events.onPhase("reconnecting");
    const delay = Math.min(
      this.policy.baseDelayMs * 2 ** (this.attempts - 1),
      this.policy.maxDelayMs,
    );
    this.reconnectTimer = window.setTimeout(() => this.connect(), delay);
  }

  private dispatch(raw: string): void {
    let msg: JsonRpcWire;
    try {
      msg = JSON.parse(raw) as JsonRpcWire;
    } catch (error) {
      console.warn("[acp] 非 JSON WS 帧，已丢弃", error);
      return;
    }
    if (typeof msg.id === "number") {
      this.settle(msg);
      return;
    }
    if (typeof msg.method === "string") {
      this.events.onNotification(msg.method, msg.params ?? null);
    }
  }

  private settle(msg: JsonRpcWire): void {
    if (typeof msg.id !== "number") return;
    const entry = this.pending.get(msg.id);
    if (!entry) return;
    this.pending.delete(msg.id);
    window.clearTimeout(entry.timer);
    if (msg.error) {
      entry.reject(new Error(JSON.stringify(msg.error)));
    } else {
      entry.resolve(msg.result ?? null);
    }
  }

  request(method: string, params?: unknown): Promise<unknown> {
    const ws = this.ws;
    if (!ws) return Promise.reject(new Error("acp-not-connected"));
    const id = this.nextId;
    this.nextId += 1;
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(() => {
        this.pending.delete(id);
        console.warn("[acp] 请求超时", method);
        reject(new Error("acp-request-timeout"));
      }, REQUEST_TIMEOUT_MS);
      this.pending.set(id, { resolve, reject, timer });
      ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params: params ?? {} }));
    });
  }

  async initialize(): Promise<InitializeResult> {
    return (await this.request("initialize", initializeParams())) as InitializeResult;
  }

  async sessionNew(cwd?: string): Promise<SessionNewResult> {
    return (await this.request("session/new", { cwd: cwd ?? null })) as SessionNewResult;
  }

  async sessionPrompt(sessionId: string, text: string): Promise<PromptResult> {
    return (await this.request("session/prompt", promptParams(sessionId, text))) as PromptResult;
  }

  sessionCancel(sessionId: string): void {
    const ws = this.ws;
    if (!ws) return;
    ws.send(JSON.stringify({ jsonrpc: "2.0", method: "session/cancel", params: { sessionId } }));
  }

  async sessionList(): Promise<SessionListResult> {
    return (await this.request("session/list", {})) as SessionListResult;
  }

  async sessionResume(sessionId: string): Promise<SessionSummary> {
    return (await this.request("session/resume", { sessionId })) as SessionSummary;
  }

  async sessionClose(sessionId: string): Promise<void> {
    await this.request("session/close", { sessionId });
  }

  /** 用户主动断开：不触发重连 */
  close(): void {
    this.userClosed = true;
    this.clearReconnectTimer();
    const ws = this.ws;
    this.ws = null;
    this.attempts = 0;
    if (ws) ws.close(1000, "client-close");
    this.events.onPhase("idle");
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      window.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}
