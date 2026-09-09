// A2A 任务相连接（a2a-over-p2p-design §5.2，拍板 Q10）：1 task = 1 流。
// console 泵 proto=a2a 为字节透传——每条 WS 独占一条 /a2a/1 流，故每个任务
// 开独立 WS，首帧 tasks/create 建附；后续 tasks/send|cancel 复用同连接，
// tasks/message|status 通知经回调上抛（store 按 taskId 路由入簿）。
import { decodeFrame, NdjsonAssembler } from "@/acp/ndjson";
import { resolveWsFactory, type WsLike } from "@/acp/ws-factory";

import type { A2aTaskStatusNotice, A2aTaskMessageNotice } from "./task-types";

export type TaskNotice = A2aTaskStatusNotice | A2aTaskMessageNotice;

export interface TaskSocketCallbacks {
  onNotice: (notice: TaskNotice) => void;
  /** 连接面失败（error/close 且任务未达终态由 store 决定语义）；reason 可读。 */
  onClosed: (reason: string) => void;
}

const REQUEST_TIMEOUT_MS = 30000;

export function taskChannelUrl(wsUrl: string, token: string, hostPeer: string): string {
  return (
    wsUrl.replace(/\/+$/, "") +
    "/?token=" + encodeURIComponent(token) +
    "&peer=" + encodeURIComponent(hostPeer) +
    "&proto=a2a"
  );
}

interface JsonRpcErrorShape {
  code?: unknown;
  message?: unknown;
}

function errorText(error: JsonRpcErrorShape): string {
  const code = typeof error.code === "number" ? error.code : 0;
  const message = typeof error.message === "string" ? error.message : "unknown";
  return code + " " + message;
}

export class TaskSocket {
  private ws: WsLike | null = null;
  private readonly assembler = new NdjsonAssembler();
  private nextId = 1;
  private readonly pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (reason: Error) => void; timer: ReturnType<typeof setTimeout> }
  >();
  private closedByUs = false;

  private constructor(private readonly cb: TaskSocketCallbacks) {}

  /** 建连；onopen 前请求排队由调用方 await open() 保证首帧为 tasks/create。 */
  static open(url: string, cb: TaskSocketCallbacks): TaskSocket {
    const socket = new TaskSocket(cb);
    const ws = resolveWsFactory()(url);
    socket.ws = ws;
    ws.onopen = () => socket.settleOpen();
    ws.onmessage = (ev) => {
      void decodeFrame(ev.data)
        .then((text) => {
          for (const line of socket.assembler.push(text)) socket.dispatch(line);
        })
        .catch((error) => console.warn("[a2a] 任务流帧解码失败", error));
    };
    ws.onerror = (ev) => {
      console.warn("[a2a] 任务流连接错误", ev?.message);
      if (!socket.closedByUs) socket.failAll(new Error(ev?.message ?? "任务流连接错误"));
    };
    ws.onclose = () => {
      socket.ws = null;
      socket.failAll(new Error("任务流已关闭"));
      socket.cb.onClosed(socket.closedByUs ? "closed by client" : "任务流已关闭");
    };
    return socket;
  }

  private openWaiters: Array<() => void> = [];
  private openError: Error | null = null;
  private openErrorWaiters: Array<(error: Error) => void> = [];
  private opened = false;

  private settleOpen(): void {
    this.opened = true;
    for (const w of this.openWaiters.splice(0)) w();
  }

  private failAll(reason: Error): void {
    this.openError = reason;
    for (const w of this.openErrorWaiters.splice(0)) w(reason);
    for (const entry of this.pending.values()) {
      clearTimeout(entry.timer);
      entry.reject(reason);
    }
    this.pending.clear();
  }

  /** 等待连接就绪；建连失败/超时抛错（调用方显式上浮，禁静默）。 */
  awaitOpen(timeoutMs = 10000): Promise<void> {
    if (this.opened) return Promise.resolve();
    if (this.openError) return Promise.reject(this.openError);
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        const idx = this.openWaiters.indexOf(resume);
        if (idx >= 0) this.openWaiters.splice(idx, 1);
        reject(new Error("任务流连接超时"));
      }, timeoutMs);
      const resume = () => {
        clearTimeout(timer);
        resolve();
      };
      this.openWaiters.push(resume);
      this.openErrorWaiters.push((error) => {
        clearTimeout(timer);
        reject(error);
      });
    });
  }

  /** JSON-RPC 请求；应答 id 配对，error 帧拒绝（原文上浮），超时兜底。 */
  request(method: string, params: Record<string, unknown>): Promise<unknown> {
    const ws = this.ws;
    if (!ws) return Promise.reject(new Error("任务流未连接"));
    const id = this.nextId++;
    return new Promise<unknown>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("任务流请求超时"));
      }, REQUEST_TIMEOUT_MS);
      this.pending.set(id, { resolve, reject, timer });
      ws.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
    });
  }

  close(): void {
    this.closedByUs = true;
    this.ws?.close(1000, "task done");
    this.ws = null;
  }

  private dispatch(line: string): void {
    let frame: Record<string, unknown>;
    try {
      frame = JSON.parse(line) as Record<string, unknown>;
    } catch {
      console.warn("[a2a] 任务流收到非 JSON 行（丢弃）");
      return;
    }
    if (typeof frame.id === "number" && (frame.result !== undefined || frame.error !== undefined)) {
      const entry = this.pending.get(frame.id);
      if (!entry) return;
      this.pending.delete(frame.id);
      clearTimeout(entry.timer);
      if (frame.error && typeof frame.error === "object") {
        entry.reject(new Error(errorText(frame.error as JsonRpcErrorShape)));
      } else {
        entry.resolve(frame.result);
      }
      return;
    }
    if (frame.method !== "tasks/message" && frame.method !== "tasks/status") return;
    const params = (frame.params ?? {}) as Record<string, unknown>;
    if (typeof params.taskId !== "string") return;
    if (frame.method === "tasks/status") {
      this.cb.onNotice(params as unknown as A2aTaskStatusNotice);
    } else {
      this.cb.onNotice(params as unknown as A2aTaskMessageNotice);
    }
  }
}
