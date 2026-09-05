// mock WS 行为单测：关断码与时序按 2026-09-05 真机对拍实测断言
//（docs/notes/2026-09-05-acp-real-calibration.md 对拍矩阵 R3a/b/h/i）。
import { describe, expect, it, vi } from "vitest";

import { mockAcpConsole, MockSocket } from "./mock-acp-ws";

interface WireFrame {
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { message: string };
}

function connect(peer = "mock-peer", token = "mock-token"): MockSocket {
  const url = "ws://127.0.0.1:1/?token=" + token + "&peer=" + peer;
  return new MockSocket(url, mockAcpConsole);
}

function decode(ev: { data: unknown }): WireFrame {
  return JSON.parse(new TextDecoder().decode(ev.data as Uint8Array)) as WireFrame;
}

function opened(peer?: string, token?: string): Promise<MockSocket> {
  return new Promise((resolve, reject) => {
    const socket = connect(peer, token);
    socket.onopen = () => resolve(socket);
    socket.onclose = (ev) => reject(new Error("closed:" + ev.code));
  });
}

function tick(ms = 30): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

describe("mock acp console", () => {
  it("错 token：HTTP 401 升级拒绝，客户端视角 error + 1006 空 reason", async () => {
    const events: string[] = [];
    const bad = connect("mock-peer", "nope");
    bad.onerror = () => events.push("error");
    bad.onclose = (ev) => events.push("close:" + ev.code + ":" + ev.reason);
    await tick();
    expect(events).toEqual(["error", "close:1006:"]);
  });

  it("未知 peer：先 open 后 Close(4500)（console 先 accept 再拨号）", async () => {
    const events: string[] = [];
    const unknown = connect("stranger");
    unknown.onopen = () => events.push("open");
    unknown.onclose = (ev) => events.push("close:" + ev.code + ":" + ev.reason);
    await tick();
    expect(events).toEqual(["open", "close:4500:dial-failed"]);
  });

  it("agent 桥拒绝握手：Close(4403, denied:<code>)", async () => {
    mockAcpConsole.reset();
    mockAcpConsole.configure({ deniedPeers: ["suspicious"] });
    const closes: Array<{ code: number; reason: string }> = [];
    const socket = connect("suspicious");
    socket.onclose = (ev) => closes.push(ev);
    await tick();
    expect(closes).toEqual([{ code: 4403, reason: "denied:peer-not-allowed" }]);
  });

  it("对端死亡：dropAll 默认 1006 空 reason（console 无 Close 帧）", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const closes: Array<{ code: number; reason: string }> = [];
    socket.onclose = (ev) => closes.push(ev);
    mockAcpConsole.dropAll();
    expect(closes).toEqual([{ code: 1006, reason: "" }]);
  });

  it("合帧入口：两条通知并入一个二进制帧（console 64KiB 块行为）", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const frames: string[] = [];
    socket.onmessage = (ev) => frames.push(new TextDecoder().decode(ev.data as Uint8Array));
    socket.serverPushCoalesced([
      { method: "session/update", params: { sessionId: "s-001", update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "a" } } } },
      { method: "session/update", params: { sessionId: "s-001", update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "b" } } } },
    ]);
    expect(frames).toHaveLength(1);
    expect(frames[0].split("\n").filter((l) => l.trim().length > 0)).toHaveLength(2);
    socket.close();
  });

  it("initialize/session/list 全链回放（二进制帧 + 行尾换行）", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const pending = new Promise<string>((resolve) => {
      socket.onmessage = (ev) => {
        const msg = decode(ev);
        if (msg.id === 2) resolve(JSON.stringify(msg.result));
      };
    });
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} }) + "\n");
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 2, method: "session/list", params: {} }) + "\n");
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
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "session/new", params: {} }) + "\n");
    const results: Array<{ id: number; result?: { stopReason?: string } }> = [];
    socket.onmessage = (ev) => {
      const msg = decode(ev);
      if (typeof msg.id === "number") results.push(msg as { id: number });
    };
    await tick();
    socket.send(
      JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "session/prompt",
        params: { sessionId: "s-001", prompt: [{ type: "text", text: "go" }] },
      }) + "\n",
    );
    await tick();
    socket.send(JSON.stringify({ jsonrpc: "2.0", method: "session/cancel", params: { sessionId: "s-001" } }) + "\n");
    await vi.waitFor(() => {
      const prompt = results.find((r) => r.id === 2);
      expect(prompt?.result && (prompt.result as { stopReason?: string }).stopReason).toBe("cancelled");
    });
    socket.close();
  });

  it("未知方法回 JSON-RPC 错误", async () => {
    const socket = await opened();
    const pending = new Promise<string>((resolve) => {
      socket.onmessage = (ev) => {
        const msg = decode(ev);
        if (msg.id === 9) resolve(msg.error?.message ?? "");
      };
    });
    socket.send(JSON.stringify({ jsonrpc: "2.0", id: 9, method: "nope", params: {} }) + "\n");
    await expect(pending).resolves.toContain("method not found");
    socket.close();
  });

  it("set_config_option 更新目录并回完整配置态；未知项报错", async () => {
    mockAcpConsole.reset();
    const socket = await opened();
    const results: WireFrame[] = [];
    socket.onmessage = (ev) => {
      const msg = decode(ev);
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
        const msg = decode(ev);
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
