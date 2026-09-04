// AcpConnection 单测：注入假 socket 验证 JSON-RPC 关联、通知分发、
// 关闭码分类与自动重连迁移（不依赖 jsdom 外的网络）。
import { describe, expect, it } from "vitest";

import { AcpConnection, type AcpConnectionEvents, type ReconnectPolicy } from "./acp-connection";
import type { WsLike, WebSocketFactory } from "./ws-factory";

class FakeSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  readonly sent: string[] = [];
  constructor(readonly url: string) {}
  send(data: string): void {
    this.sent.push(data);
  }
  close(code = 1000, reason = ""): void {
    this.onclose?.({ code, reason });
  }
  serverOpen(): void {
    this.onopen?.();
  }
  serverMessage(msg: unknown): void {
    this.onmessage?.({ data: JSON.stringify(msg) + "\n" });
  }
  serverClose(code: number, reason: string): void {
    this.onclose?.({ code, reason });
  }
}

function harness(policy?: ReconnectPolicy) {
  const sockets: FakeSocket[] = [];
  const factory: WebSocketFactory = (url) => {
    const socket = new FakeSocket(url);
    sockets.push(socket);
    return socket;
  };
  const phases: string[] = [];
  const notifications: Array<{ method: string; params: unknown }> = [];
  const closes: Array<{ kind: string; code: number }> = [];
  const reconnects: number[] = [];
  const events: AcpConnectionEvents = {
    onPhase: (p) => phases.push(p),
    onNotification: (method, params) => notifications.push({ method, params }),
    onCloseInfo: (info) => closes.push({ kind: info.kind, code: info.code }),
    onReconnect: (attempt) => reconnects.push(attempt),
  };
  const conn = new AcpConnection(
    { wsUrl: "ws://127.0.0.1:1", token: "tok", peer: "peer-x" },
    factory,
    events,
    policy,
  );
  return { conn, sockets, phases, notifications, closes, reconnects };
}

const FAST_POLICY: ReconnectPolicy = { maxAttempts: 2, baseDelayMs: 5, maxDelayMs: 10 };

function last<T>(list: T[]): T {
  return list[list.length - 1];
}

describe("AcpConnection", () => {
  it("request 结算：按 id 关联响应，参数带 token/peer 查询串", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const pending = h.conn.request("session/list", {});
    const frame = JSON.parse(last(h.sockets[0].sent));
    expect(frame.id).toBe(1);
    expect(h.sockets[0].url).toContain("token=tok");
    expect(h.sockets[0].url).toContain("peer=peer-x");
    h.sockets[0].serverMessage({ jsonrpc: "2.0", id: 1, result: { sessions: [] } });
    await expect(pending).resolves.toEqual({ sessions: [] });
  });

  it("notification 分发到事件面", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    h.sockets[0].serverMessage({
      jsonrpc: "2.0",
      method: "session/update",
      params: { sessionId: "s-1", update: { sessionUpdate: "agent_message_chunk" } },
    });
    await new Promise((r) => setTimeout(r, 0));
    expect(h.notifications).toHaveLength(1);
    expect(h.notifications[0].method).toBe("session/update");
  });

  it("4403 关闭归类 denied 且不触发重连", () => {
    const h = harness(FAST_POLICY);
    h.conn.connect();
    h.sockets[0].serverClose(4403, "denied:bad-token");
    expect(last(h.closes)).toEqual({ kind: "denied", code: 4403 });
    expect(last(h.phases)).toBe("offline");
    expect(h.reconnects).toHaveLength(0);
  });

  it("4500 关闭归类 dial-failed 且不触发重连", () => {
    const h = harness(FAST_POLICY);
    h.conn.connect();
    h.sockets[0].serverClose(4500, "dial-failed");
    expect(last(h.closes)).toEqual({ kind: "dial-failed", code: 4500 });
    expect(last(h.phases)).toBe("offline");
  });

  it("意外断流自动重连：reconnecting 后重拨成功回 online", async () => {
    const h = harness(FAST_POLICY);
    h.conn.connect();
    h.sockets[0].serverOpen();
    h.sockets[0].serverClose(1000, "agent-stream-dropped");
    expect(last(h.phases)).toBe("reconnecting");
    expect(h.reconnects).toEqual([1]);
    await new Promise((r) => setTimeout(r, 20));
    expect(h.sockets).toHaveLength(2);
    h.sockets[1].serverOpen();
    expect(last(h.phases)).toBe("online");
  });

  it("重连次数用尽转 offline", async () => {
    const h = harness({ maxAttempts: 1, baseDelayMs: 5, maxDelayMs: 5 });
    h.conn.connect();
    h.sockets[0].serverOpen();
    h.sockets[0].serverClose(1000, "dropped");
    await new Promise((r) => setTimeout(r, 15));
    h.sockets[1].serverClose(1000, "dropped");
    expect(last(h.phases)).toBe("offline");
    expect(h.reconnects).toEqual([1]);
  });

  it("用户主动 close 不重连，phase 回 idle", () => {
    const h = harness(FAST_POLICY);
    h.conn.connect();
    h.sockets[0].serverOpen();
    h.conn.close();
    expect(last(h.phases)).toBe("idle");
    expect(h.reconnects).toHaveLength(0);
  });

  it("上行请求帧尾带换行（agent 侧行界依赖，真机对拍 R3i）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const pending = h.conn.request("initialize", {});
    expect(last(h.sockets[0].sent)).toMatch(/\n$/);
    h.sockets[0].serverMessage({ jsonrpc: "2.0", id: 1, result: {} });
    await pending;
  });

  it("下行二进制帧一帧多行：全部按行派发（合帧实测 R3i）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const line = JSON.stringify({
      jsonrpc: "2.0",
      method: "session/update",
      params: { sessionId: "s-1", update: { sessionUpdate: "agent_message_chunk" } },
    });
    h.sockets[0].onmessage?.({ data: new TextEncoder().encode(line + "\n" + line + "\n") });
    await new Promise((r) => setTimeout(r, 0));
    expect(h.notifications).toHaveLength(2);
  });

  it("残行跨帧：半行不派发，补齐行界后派发（大行拆帧实测 R3i）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const full = JSON.stringify({ jsonrpc: "2.0", method: "session/update", params: {} });
    h.sockets[0].onmessage?.({ data: full.slice(0, Math.floor(full.length / 2)) });
    expect(h.notifications).toHaveLength(0);
    h.sockets[0].onmessage?.({ data: full.slice(Math.floor(full.length / 2)) + "\n" });
    await new Promise((r) => setTimeout(r, 0));
    expect(h.notifications).toHaveLength(1);
  });

  it("Blob 帧解码后派发（binaryType=blob 浏览器默认形态）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const line = JSON.stringify({ jsonrpc: "2.0", method: "session/update", params: {} });
    h.sockets[0].onmessage?.({ data: new Blob([line + "\n"]) });
    await new Promise((r) => setTimeout(r, 10));
    expect(h.notifications).toHaveLength(1);
  });

  it("1006 空 reason（401 升级拒绝实测形态）归类 abnormal 并触发重连", () => {
    const h = harness(FAST_POLICY);
    h.conn.connect();
    h.sockets[0].serverClose(1006, "");
    expect(last(h.closes)).toEqual({ kind: "abnormal", code: 1006 });
    expect(last(h.phases)).toBe("reconnecting");
    expect(h.reconnects).toEqual([1]);
  });

  it("session/delete 方法面（ACP v1 契约，替换 mock 期 session/close）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[0].serverOpen();
    const pending = h.conn.sessionDelete("s-9");
    const frame = JSON.parse(last(h.sockets[0].sent));
    expect(frame.method).toBe("session/delete");
    expect(frame.params).toEqual({ sessionId: "s-9" });
    h.sockets[0].serverMessage({ jsonrpc: "2.0", id: 1, result: {} });
    await pending;
  });
});
