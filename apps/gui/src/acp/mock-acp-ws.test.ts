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

  it("set_config_option 更新目录并回完整配置态；未知项报错", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const results: Array<{ id: number; result?: unknown; error?: unknown }> = [];
    socket.onmessage = (ev) => {
      const msg = JSON.parse(String(ev.data));
      if (typeof msg.id === "number") results.push(msg);
    };
    socket.send(JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "session/set_config_option",
      params: { configId: "model", value: "mock-model-b" },
    }));
    await vi.waitFor(() => expect(results).toHaveLength(1));
    const options = (results[0].result as { configOptions: Array<{ id: string; currentValue: string }> })
      .configOptions;
    expect(options.find((o) => o.id === "model")?.currentValue).toBe("mock-model-b");
    socket.send(JSON.stringify({
      jsonrpc: "2.0",
      id: 2,
      method: "session/set_config_option",
      params: { configId: "nope", value: "x" },
    }));
    await vi.waitFor(() => expect(results).toHaveLength(2));
    expect(results[1].error).toBeTruthy();
    socket.close();
  });

  it("客户端应答帧（无 method 带 id）登记进 responses 供断言", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 42, result: { outcome: { outcome: "cancelled" } } }));
    await vi.waitFor(() => expect(mockAcpConsole.responses).toHaveLength(1));
    expect(mockAcpConsole.responses[0]).toMatchObject({ id: 42 });
    socket.close();
  });

  it("pushReattach 广播桥约定补放通知（无 id）", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const seen = new Promise<{ method: string; params: unknown }>((resolve) => {
      socket.onmessage = (ev) => {
        const msg = JSON.parse(String(ev.data));
        if (msg.method === "dsh/bridge/reattach") resolve({ method: msg.method, params: msg.params });
      };
    });
    mockAcpConsole.pushReattach(3);
    const frame = await seen;
    expect(frame.params).toEqual({ replayed: 3 });
    socket.close();
  });

  it("emitDiscovery 推快照给已接线的 sink", () => {
    mockAcpConsole.reset();
    const received: Array<{ peer: string }> = [];
    mockAcpConsole.discoveryPeers = [{ peer: "peer-d1", addrs: ["/ip4/10.0.0.8/tcp/4001"] }];
    mockAcpConsole.onDiscovery = (peers) => {
      for (const p of peers) received.push({ peer: p.peer });
    };
    mockAcpConsole.emitDiscovery();
    expect(received).toEqual([{ peer: "peer-d1" }]);
  });
});
