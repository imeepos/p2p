// 可脚本化的本地 WS mock：dev（VITE_MOCK_IPC=1）与测试共用同一实现。
// 关断码与时序按 2026-09-05 真机对拍实测对齐（docs/notes/
// 2026-09-05-acp-real-calibration.md）：错 token = HTTP 升级层 401 拒绝
// （客户端视角 error + 1006 空 reason）、未知 peer = 先 open 后
// Close(4500)、agent 桥拒绝握手 = Close(4403, "denied:<code>")、对端死亡 =
// 无 Close 帧 1006；下行帧为二进制（Uint8Array）且行界带行尾换行，可多行
// 合帧。reattach 补放通知与 request_permission 透传对齐
// apps/acp-agent/README.md 桥约定。
import type { ConfigOption } from "./protocol";
import type { WsLike, WebSocketFactory } from "./ws-factory";
import {
  MOCK_AGENT_INFO,
  permissionRequestFrame,
  stepToUpdate,
  type MockPromptStep,
} from "./mock-script";
import { mockAcpConsole, type MockAcpConsole } from "./mock-console";

export { mockAcpConsole, type DiscoverySink } from "./mock-console";

interface WireMessage {
  jsonrpc?: string;
  id?: number;
  method?: string;
  params?: Record<string, unknown>;
  result?: unknown;
  error?: { code: number; message: string };
}

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
    if (typeof msg.method !== "string") {
      // 客户端应答帧：登记供断言（request_permission outcome 等）
      if (typeof msg.id === "number") {
        this.box.responses.push({ id: msg.id, result: msg.result, error: msg.error });
      }
      return;
    }
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
      case "session/resume": {
        const resumeId = String(params.sessionId ?? "");
        if (!this.box.sessions.has(resumeId)) {
          this.replyError(msg.id, -32002, "session not found");
          return;
        }
        this.reply(msg.id, { sessionId: resumeId });
        return;
      }
      case "session/delete":
        this.handleCloseSession(msg.id, params);
        return;
      case "session/set_config_option":
        this.handleSetConfigOption(msg.id, params);
        return;
      default:
        this.replyError(msg.id, -32601, "method not found: " + msg.method);
    }
  }

  private handleCloseSession(id: number, params: Record<string, unknown>): void {
    const sessionId = String(params.sessionId ?? "");
    this.box.sessions.delete(sessionId);
    this.reply(id, {});
  }

  private handleNew(id: number): void {
    const sessionId = this.box.nextSessionId();
    this.box.sessions.set(sessionId, { sessionId, title: "session " + sessionId });
    this.reply(id, { sessionId, configOptions: this.currentConfigOptions() });
  }

  private currentConfigOptions(): ConfigOption[] {
    return this.box.config.configOptions.map((o) => ({ ...o }));
  }

  private handleSetConfigOption(id: number, params: Record<string, unknown>): void {
    const configId = String(params.configId ?? "");
    const value = params.value;
    const target = this.box.config.configOptions.find((o) => o.id === configId);
    if (!target || typeof value !== "string") {
      this.replyError(id, -32602, "unknown config option: " + configId);
      return;
    }
    target.currentValue = value;
    this.reply(id, { configOptions: this.currentConfigOptions() });
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
    if (step.kind === "permission") {
      this.emitPermission(pending.sessionId, step);
      return;
    }
    const frame = stepToUpdate(step, pending.sessionId);
    if (frame) this.box.broadcast(frame.method, frame.params);
  }

  private emitPermission(
    sessionId: string,
    step: Extract<MockPromptStep, { kind: "permission" }>,
  ): void {
    const id = this.box.permissionSeq;
    this.box.permissionSeq += 1;
    this.box.permissionRequests.push({ id, sessionId, toolKind: step.toolKind });
    this.deliver(permissionRequestFrame(id, sessionId, step) as WireMessage);
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
