// mock WS 行为单测：对齐 acp-console 契约的关断码与 ACP 方法面回放。
import { describe, expect, it, vi } from "vitest";

import { mockAcpConsole, MockSocket } from "./mock-acp-ws";

function connect(peer = "mock-peer", token = "mock-token"): MockSocket {
  const url = "ws://127.0.0.1:1/?token=" + token + "&peer=" + peer;
  return new MockSocket(url, mockAcpConsole);
}

function opened(peer?: string, token?: string): Promise<MockSocket> {
  return new Promise((resolve, reject) => {
    const socket = connect(peer, token);
    socket.onopen = () => resolve(socket);
    socket.onclose = (ev) => reject(new Error("closed:" + ev.code));
  });
}

describe("mock acp console", () => {
  it("token 错误 4403 拒绝，未知 peer 4500 拒绝", async () => {
    const closes: number[] = [];
    const bad = connect("mock-peer", "nope");
    const unknown = connect("stranger", "mock-token");
    bad.onclose = (ev) => closes.push(ev.code);
    unknown.onclose = (ev) => closes.push(ev.code);
    await new Promise((r) => setTimeout(r, 30));
    expect(closes.sort()).toEqual([4403, 4500]);
  });

  it("initialize/session/new/session/list 全链回放", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const pending = new Promise<string>((resolve) => {
      socket.onmessage = (ev) => {
        const msg = JSON.parse(String(ev.data));
        if (msg.id === 2) resolve(JSON.stringify(msg.result));
      };
    });
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} }));
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 2, method: "session/list", params: {} }));
    const list = JSON.parse(await pending);
    expect(list.sessions).toEqual([]);
    socket.close();
  });

  it("session/cancel 通知使 prompt 以 cancelled 结算", async () => {
    mockAcpConsole.reset();
    mockAcpConsole.configure({
      promptScript: [
        { kind: "message", text: "long" },
        { kind: "stop", reason: "end_turn" },
      ],
      chunkDelayMs: 50,
    });
    const socket = await opened();
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "session/new", params: {} }));
    const results: Array<{ id: number; result?: { stopReason?: string } }> = [];
    socket.onmessage = (ev) => {
      const msg = JSON.parse(String(ev.data));
      if (typeof msg.id === "number") results.push(msg);
    };
    await new Promise((r) => setTimeout(r, 30));
    socket.send(
      JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "session/prompt",
        params: { sessionId: "s-001", prompt: [{ type: "text", text: "go" }] },
      }),
    );
    await new Promise((r) => setTimeout(r, 30));
    socket.send(JSON.stringify({ jsonrpc: "2.0", method: "session/cancel", params: { sessionId: "s-001" } }));
    await vi.waitFor(() => {
      expect(results.find((r) => r.id === 2)?.result?.stopReason).toBe("cancelled");
    });
    socket.close();
  });

  it("未知方法回 JSON-RPC 错误", async () => {
    const socket = await opened();
    const pending = new Promise<string>((resolve) => {
      socket.onmessage = (ev) => {
        const msg = JSON.parse(String(ev.data));
        if (msg.id === 9) resolve(msg.error.message);
      };
    });
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 9, method: "nope", params: {} }));
    await expect(pending).resolves.toContain("method not found");
    socket.close();
  });
});
