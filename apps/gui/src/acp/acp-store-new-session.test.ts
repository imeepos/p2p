// AF1 新建会话反馈闭环：store 单飞与 pending 契约。
// 根因：ACP initialize/sessionNew 最坏 120s，无单飞时连点重复发 IPC（计划 2026-09-14）。
// socket 应答手动可控（setTimeout 出队），时序断言不靠竞态运气。
import { waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WsLike, WebSocketFactory } from "./ws-factory";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { useAcpStore } = await import("./acp-store");
const { setWsFactory } = await import("./ws-factory");
const { MOCK_AGENT_INFO } = await import("./mock-script");
const { resetToastDedupForTest } = await import("@/components/feedback/toast");
await import("@/i18n");

/** 应答脚本化 socket：initialize/session/list 恒成功，session/new 由 failNew 控制 */
class ScriptedSocket implements WsLike {
  /** 上行帧方法记录：断言 session/new 只发一次 */
  static methods: string[] = [];
  static failNew = false;
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;

  constructor() {
    window.setTimeout(() => this.onopen?.(), 0);
  }

  send(data: string): void {
    for (const line of data.split("\n")) {
      if (!line.trim()) continue;
      const msg = JSON.parse(line) as { id?: number; method?: string };
      if (typeof msg.id !== "number" || !msg.method) continue;
      ScriptedSocket.methods.push(msg.method);
      this.dispatch(msg.id, msg.method);
    }
  }

  private dispatch(id: number, method: string): void {
    if (method === "initialize") {
      this.deferReply(id, MOCK_AGENT_INFO);
      return;
    }
    if (method === "session/new") {
      if (ScriptedSocket.failNew) {
        this.deferError(id, "mock session/new rejected");
      } else {
        this.deferReply(id, { sessionId: "s-t-" + id });
      }
      return;
    }
    if (method === "session/list") {
      this.deferReply(id, { sessions: [] });
      return;
    }
    this.deferError(id, "unexpected method: " + method);
  }

  private deferReply(id: number, result: unknown): void {
    window.setTimeout(() => this.deliver({ jsonrpc: "2.0", id, result }), 0);
  }

  private deferError(id: number, message: string): void {
    window.setTimeout(
      () => this.deliver({ jsonrpc: "2.0", id, error: { code: -32000, message } }),
      0,
    );
  }

  private deliver(msg: unknown): void {
    this.onmessage?.({ data: new TextEncoder().encode(JSON.stringify(msg) + "\n") });
  }

  close(): void {
    this.onclose?.({ code: 1000, reason: "client-close" });
  }
}

function freshFactory(): WebSocketFactory {
  return () => new ScriptedSocket();
}

async function connectOnline(): Promise<void> {
  setWsFactory(freshFactory());
  useAcpStore.setState({ draft: { wsUrl: "ws://127.0.0.1:1", token: "t", peer: "p" } });
  useAcpStore.getState().connect();
  await waitFor(() => {
    expect(useAcpStore.getState().phase).toBe("online");
  });
}

function newSessionFrames(): number {
  return ScriptedSocket.methods.filter((m) => m === "session/new").length;
}

beforeEach(() => {
  ScriptedSocket.methods = [];
  ScriptedSocket.failNew = false;
  resetToastDedupForTest();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ phase: "idle", reconnect: null });
});

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
});

describe("acp-store newSession 单飞与 pending", () => {
  it("in-flight 期间重复调用返回同一 promise，session/new 只发一帧", async () => {
    await connectOnline();
    const store = useAcpStore.getState();
    const first = store.newSession();
    const second = store.newSession();
    expect(second).toBe(first);
    await first;
    await waitFor(() => {
      expect(useAcpStore.getState().newSessionPending).toBe(false);
    });
    expect(newSessionFrames()).toBe(1);
  });

  it("pending 布尔：调用即置 true，成功后复位 false", async () => {
    await connectOnline();
    const flight = useAcpStore.getState().newSession();
    expect(useAcpStore.getState().newSessionPending).toBe(true);
    await flight;
    expect(useAcpStore.getState().newSessionPending).toBe(false);
  });

  it("失败路径：resolve false（不 reject）、pending 复位、lastError 记码", async () => {
    await connectOnline();
    ScriptedSocket.failNew = true;
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      await expect(useAcpStore.getState().newSession()).resolves.toBe(false);
    } finally {
      warnSpy.mockRestore();
    }
    expect(useAcpStore.getState().newSessionPending).toBe(false);
    expect(useAcpStore.getState().lastError).toBe("sessionNewFailed");
  });

  it("上次失败的残留 lastError 不得把本次成功误判为失败", async () => {
    await connectOnline();
    ScriptedSocket.failNew = true;
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    await useAcpStore.getState().newSession();
    ScriptedSocket.failNew = false;
    await expect(useAcpStore.getState().newSession()).resolves.toBe(true);
    warnSpy.mockRestore();
  });
});
