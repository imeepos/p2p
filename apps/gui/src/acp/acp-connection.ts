// ACP 客户端薄层：JSON-RPC over 本地 WS 的最小封装。
// console 是纯字节泵，本层即 GUI 的 ACP 协议智能所在；下行按 ndjson 行界
// 重组（一帧可多行、一行可跨帧，真机对拍 R3i），上行行尾补换行；断线自动
// 重连并在事件面给出可观测提示（阶段迁移 + 关闭码分类），不做静默重试。
import {
  buildWsUrl,
  classifyClose,
  initializeParams,
  promptParams,
  type AcpCloseInfo,
  type ConfigOptionsResult,
  type AcpEndpoint,
  type AcpPhase,
  type InitializeResult,
  type PromptResult,
  type SessionListResult,
  type SessionNewResult,
  type SessionSummary,
} from "./protocol";
import { decodeFrame, NdjsonAssembler } from "./ndjson";
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
  /** agent 侧 JSON-RPC 请求（带 id，如 session/request_permission），须应答 */
  onRequest(method: string, params: unknown, id: number): void;
  onCloseInfo(info: AcpCloseInfo): void;
  onReconnect(attempt: number, maxAttempts: number): void;
}

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  /** null = 不挂通用超时（session/prompt 长回合由应答/断连结算） */
  timer: ReturnType<typeof setTimeout> | null;
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
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private attempts = 0;
  private userClosed = false;
  private assembler = new NdjsonAssembler();
  private frameChain: Promise<void> = Promise.resolve();

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
    ws.onmessage = (ev) => this.enqueueFrame(ev.data);
    ws.onerror = (ev) => {
      console.warn("[acp] WS 错误信号", ev.message);
    };
    ws.onclose = (ev) => this.handleClose(ev.code, ev.reason);
  }

  private handleClose(code: number, reason: string): void {
    this.ws = null;
    for (const [, p] of this.pending) {
      // 环境无关定时器 API：重连定时器可能在宿主 window 卸载后触发（测试稳定性）
      if (p.timer) clearTimeout(p.timer);
      p.reject(new Error("acp-connection-closed"));
    }
    this.pending.clear();
    const info = classifyClose(code, reason);
    this.events.onCloseInfo(info);
    // 真机对拍 R3a：401 升级拒绝客户端视角为 1006 空 reason——重连必同败，首轮即停
    if (info.kind === "abnormal" && info.code === 1006 && info.reason === "") {
      console.warn("[acp] 1006 无原因关闭（疑鉴权拒绝），不进重连计数转 offline");
      this.events.onPhase("offline");
      return;
    }
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
    this.reconnectTimer = setTimeout(() => this.connect(), delay);
  }

  /** 帧解码按到达序串行（Blob 路径异步）；行界重组后逐行分发 */
  private enqueueFrame(data: unknown): void {
    this.frameChain = this.frameChain
      .then(async () => this.assembler.push(await decodeFrame(data)))
      .then((lines) => {
        for (const line of lines) {
          if (line.trim().length > 0) this.dispatch(line);
        }
      })
      .catch((error) => console.warn("[acp] 帧解码失败，已丢弃", error));
  }

  private dispatch(raw: string): void {
    let msg: JsonRpcWire;
    try {
      msg = JSON.parse(raw) as JsonRpcWire;
    } catch (error) {
      console.warn("[acp] 非 JSON WS 帧，已丢弃", error);
      return;
    }
    if (typeof msg.method === "string") {
      // 带 id 的 method 帧是 agent 侧请求（响应帧永不携带 method）
      if (typeof msg.id === "number") {
        this.events.onRequest(msg.method, msg.params ?? null, msg.id);
        return;
      }
      this.events.onNotification(msg.method, msg.params ?? null);
      return;
    }
    if (typeof msg.id === "number") {
      this.settle(msg);
    }
  }

  private settle(msg: JsonRpcWire): void {
    if (typeof msg.id !== "number") return;
    const entry = this.pending.get(msg.id);
    if (!entry) return;
    this.pending.delete(msg.id);
    if (entry.timer) clearTimeout(entry.timer);
    if (msg.error) {
      entry.reject(new Error(JSON.stringify(msg.error)));
    } else {
      entry.resolve(msg.result ?? null);
    }
  }

  request(
    method: string,
    params?: unknown,
    opts?: { timeoutMs?: number | null },
  ): Promise<unknown> {
    const ws = this.ws;
    if (!ws) return Promise.reject(new Error("acp-not-connected"));
    const id = this.nextId;
    this.nextId += 1;
    const timeoutMs = opts && "timeoutMs" in opts ? opts.timeoutMs : REQUEST_TIMEOUT_MS;
    return new Promise((resolve, reject) => {
      const timer =
        timeoutMs === null
          ? null
          : setTimeout(() => {
              this.pending.delete(id);
              console.warn("[acp] 请求超时", method);
              reject(new Error("acp-request-timeout"));
            }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      // ACP ndjson 行界：console 是字节泵，行尾不带 \n 会被 agent 侧行重组器挂起
      ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params: params ?? {} }) + "\n");
    });
  }

  /** 应答 agent 侧请求（request_permission 等）；未连接时留告警不静默丢 */
  respond(id: number, result: unknown): void {
    const ws = this.ws;
    if (!ws) {
      console.warn("[acp] 未连接，无法应答 agent 请求 id=", id);
      return;
    }
    ws.send(JSON.stringify({ jsonrpc: "2.0", id, result }));
  }

  async initialize(): Promise<InitializeResult> {
    return (await this.request("initialize", initializeParams())) as InitializeResult;
  }

  async setConfigOption(
    sessionId: string,
    configId: string,
    value: string | boolean,
  ): Promise<ConfigOptionsResult> {
    const params = { sessionId, configId, value };
    return (await this.request("session/set_config_option", params)) as ConfigOptionsResult;
  }

  async sessionNew(cwd?: string): Promise<SessionNewResult> {
    return (await this.request("session/new", { cwd: cwd ?? null })) as SessionNewResult;
  }

  /** 长回合可能远超通用 30s：prompt 不挂通用超时，只由应答/错误/断连结算 */
  async sessionPrompt(sessionId: string, text: string): Promise<PromptResult> {
    const params = promptParams(sessionId, text);
    return (await this.request("session/prompt", params, { timeoutMs: null })) as PromptResult;
  }

  sessionCancel(sessionId: string): void {
    const ws = this.ws;
    if (!ws) return;
    ws.send(JSON.stringify({ jsonrpc: "2.0", method: "session/cancel", params: { sessionId } }) + "\n");
  }

  async sessionList(): Promise<SessionListResult> {
    return (await this.request("session/list", {})) as SessionListResult;
  }

  async sessionResume(sessionId: string): Promise<SessionSummary> {
    return (await this.request("session/resume", { sessionId })) as SessionSummary;
  }

  /** ACP v1 契约方法为 session/delete（session/close 系 mock 期假设，真机对拍后改正） */
  async sessionDelete(sessionId: string): Promise<void> {
    await this.request("session/delete", { sessionId });
  }

  /** 手动立即重试：清掉待发的自动重连定时器立刻拨号；attempts 计数语义不变 */
  retryNow(): void {
    if (this.userClosed) return;
    this.clearReconnectTimer();
    this.connect();
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
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}
