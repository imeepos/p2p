// task-socket + a2a-store 任务相回归：1 task = 1 流（Q10）——每任务独立 proto=a2a
// 连接、首帧 tasks/create 建附、通知按 taskId 路由、错误原文上抛（禁静默）。
import { beforeEach, afterEach, describe, expect, it } from "vitest";

import { setWsFactory, type WsLike } from "@/acp/ws-factory";

import { taskChannelUrl, TaskSocket } from "./task-socket";
import { useA2aStore } from "./a2a-store";
import type { A2aMessage } from "./task-types";

class FakeWs implements WsLike {
  sent: string[] = [];
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  /** 对端行为钩子：按收到的请求帧生成下行（缺省 echo 成功）。 */
  replier: (frame: { id: number; method: string; params: unknown }) => unknown[] = (frame) => [
    { jsonrpc: "2.0", id: frame.id, result: { taskId: "t-" + frame.id } },
  ];

  constructor(public url: string) {}

  send(data: string): void {
    this.sent.push(data);
    const frame = JSON.parse(data) as { id: number; method: string; params: unknown };
    queueMicrotask(() => this.emit(this.replier(frame)));
  }
  close(): void {}
  /** 测试驱动：模拟对端下行（一行或多行 ndjson）。 */
  emit(lines: unknown[]): void {
    if (lines.length === 0) return;
    const text = lines.map((l) => JSON.stringify(l)).join("\n") + "\n";
    this.onmessage?.({ data: text });
  }
  open(): void {
    queueMicrotask(() => this.onopen?.());
  }
}

const USER_MSG: A2aMessage = { role: "user", parts: [{ type: "text", text: "hi" }] };
const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

let created: FakeWs[] = [];
/** 新建 FakeWs 采纳的对端行为（测试在触发连接前置入）。 */
let nextReplier: FakeWs["replier"] | null = null;
let fakeTaskSeq = 0;

beforeEach(() => {
  created = [];
  nextReplier = null;
  fakeTaskSeq = 0;
  setWsFactory((url) => {
    const ws = new FakeWs(url);
    if (nextReplier) ws.replier = nextReplier;
    created.push(ws);
    ws.open();
    return ws;
  });
  useA2aStore.setState({ tasks: new Map(), unreadByAgent: {}, channelReady: false, lastError: null });
  useA2aStore.getState().configureChannel("ws://127.0.0.1:9/", "tok");
});

afterEach(() => {
  setWsFactory(null);
});

describe("taskChannelUrl", () => {
  it("携带 token/peer/proto=a2a 三参数", () => {
    const url = taskChannelUrl("ws://127.0.0.1:9/", "a b", "PeerX");
    expect(url).toBe("ws://127.0.0.1:9/?token=a%20b&peer=PeerX&proto=a2a");
  });
});

describe("a2a-store 任务相", () => {
  it("createTask：独立连接首帧 tasks/create，应答 taskId 入簿 + working 通知路由", async () => {
    nextReplier = (frame) => [
      { jsonrpc: "2.0", id: frame.id, result: { taskId: "t-1" } },
      { jsonrpc: "2.0", method: "tasks/status", params: { taskId: "t-1", state: "working" } },
    ];
    const id = await useA2aStore.getState().createTask("PeerHost", "agent-1", USER_MSG);
    expect(id).toBe("t-1");
    const sent = JSON.parse(created[0].sent[0]) as { method: string; params: unknown };
    expect(sent.method).toBe("tasks/create");
    expect(sent.params).toEqual({ agentId: "agent-1", message: USER_MSG });
    await flush();
    const task = useA2aStore.getState().tasks.get("t-1");
    expect(task?.state).toBe("working");
    expect(task?.messages[0]?.parts[0]).toMatchObject({ type: "text", text: "hi" });
  });

  it("sendTaskMessage：复用同任务连接（1 task = 1 流），tasks/send 参数带 taskId", async () => {
    const id = await useA2aStore.getState().createTask("PeerHost", "agent-1", USER_MSG);
    expect(created).toHaveLength(1);
    await useA2aStore
      .getState()
      .sendTaskMessage(id, { role: "user", parts: [{ type: "text", text: "again" }] });
    const sent = JSON.parse(created[0].sent[1]) as { method: string; params: Record<string, unknown> };
    expect(sent.method).toBe("tasks/send");
    expect(sent.params.taskId).toBe(id);
  });

  it("新任务新连接：第二个 createTask 另开一条 WS（泵 1 连接 = 1 流约束）", async () => {
    nextReplier = (frame) => [
      { jsonrpc: "2.0", id: frame.id, result: { taskId: "t-" + ++fakeTaskSeq } },
    ];
    await useA2aStore.getState().createTask("PeerHost", "agent-1", USER_MSG);
    await useA2aStore.getState().createTask("PeerHost", "agent-2", USER_MSG);
    expect(created).toHaveLength(2);
    expect(useA2aStore.getState().tasks.size).toBe(2);
  });

  it("agent 下行 tasks/message：按 messageId 去重并增量拼接文本", async () => {
    const id = await useA2aStore.getState().createTask("PeerHost", "agent-1", USER_MSG);
    created[0].emit([
      { jsonrpc: "2.0", method: "tasks/message", params: { taskId: id, messageId: "m1", message: { role: "agent", parts: [{ type: "text", text: "你" }] } } },
      { jsonrpc: "2.0", method: "tasks/message", params: { taskId: id, messageId: "m1", message: { role: "agent", parts: [{ type: "text", text: "好" }] } } },
    ]);
    await flush();
    const task = useA2aStore.getState().tasks.get(id);
    expect(task?.messages).toHaveLength(2);
    expect(task?.messages[1]?.parts[0]).toMatchObject({ type: "text", text: "你好" });
  });

  it("error 帧原文拒绝上抛 + lastError 留痕（禁静默）", async () => {
    nextReplier = (frame) => [
      { jsonrpc: "2.0", id: frame.id, error: { code: -32000, message: "gate-denied" } },
    ];
    await expect(
      useA2aStore.getState().createTask("PeerHost", "agent-1", USER_MSG),
    ).rejects.toThrow("gate-denied");
    expect(useA2aStore.getState().lastError).toContain("gate-denied");
  });

  it("未建任务就 send：显式报错不静默（任务连接不存在）", async () => {
    await expect(
      useA2aStore.getState().sendTaskMessage("missing-task", USER_MSG),
    ).rejects.toThrow("任务连接不存在");
  });
});

describe("TaskSocket awaitOpen 时序", () => {
  it("连接先于调用就绪：awaitOpen 立即 resolve（竞态防线）", async () => {
    const holder: { ws?: FakeWs } = {};
    setWsFactory((url) => {
      const w = new FakeWs(url);
      holder.ws = w;
      return w;
    });
    const sock = TaskSocket.open("ws://x", { onNotice: () => {}, onClosed: () => {} });
    holder.ws?.onopen?.();
    await expect(sock.awaitOpen()).resolves.toBeUndefined();
    sock.close();
  });

  it("open 前连接失败：awaitOpen 以错误拒绝", async () => {
    const holder: { ws?: FakeWs } = {};
    setWsFactory((url) => {
      const w = new FakeWs(url);
      holder.ws = w;
      return w;
    });
    const sock = TaskSocket.open("ws://x", { onNotice: () => {}, onClosed: () => {} });
    holder.ws?.onerror?.({ message: "refused" });
    await expect(sock.awaitOpen()).rejects.toThrow("refused");
    sock.close();
  });
});
