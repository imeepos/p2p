// A2A 任务管理 store（docs/design/a2a-over-p2p-design.md §5.2/§8.3）。
// task 相 JSON-RPC 2.0 语义：tasks/create|send|get|cancel；通知 tasks/status + tasks/message。
// 1 task = 1 流；GUI 经 acp-console 的 WS 透传通道发帧。

import { create } from "zustand";

import type { 
  A2aTask, 
  A2aMessage, 
  A2aTaskStatusNotice, 
  A2aTaskMessageNotice, 
  A2aTaskSnapshot,
  A2aPart
} from "./task-types";

interface A2aState {
  /** taskId → task 快照。 */
  tasks: Map<string, A2aTask>;
  /** agentKey → 未读计数。 */
  unreadByAgent: Record<string, number>;
  /** 连接的 WS 通道（proto=a2a）。 */
  ws: WebSocket | null;
  /** 连接状态。 */
  status: "connecting" | "online" | "offline";
  /** 最近错误。 */
  lastError: string | null;
  
  connect: (wsUrl: string, token: string) => void;
  disconnect: () => void;
  createTask: (hostPeer: string, agentId: string, message: A2aMessage) => Promise<string>;
  sendTaskMessage: (taskId: string, message: A2aMessage) => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  getTask: (taskId: string) => A2aTask | undefined;
  markRead: (agentKey: string) => void;
}

let nextRequestId = 1;
const pendingRequests = new Map<number, { resolve: (value: any) => void; reject: (reason: any) => void }>();

export const useA2aStore = create<A2aState>((set, get) => ({
  tasks: new Map(),
  unreadByAgent: {},
  ws: null,
  status: "offline",
  lastError: null,

  connect: (wsUrl: string, _token: string) => {
    const ws = new WebSocket(wsUrl);
    set({ ws, status: "connecting" });

    ws.onopen = () => {
      set({ status: "online", lastError: null });
    };

    ws.onmessage = (event) => {
      try {
        const frame = JSON.parse(event.data);
        handleFrame(frame, get, set);
      } catch (error) {
        console.error("[a2a] 帧解析失败:", error);
      }
    };

    ws.onclose = () => {
      set({ status: "offline", ws: null });
    };

    ws.onerror = () => {
      set({ status: "offline", lastError: "连接失败" });
    };
  },

  disconnect: () => {
    const { ws } = get();
    if (ws) {
      ws.close();
      set({ ws: null, status: "offline" });
    }
  },

  createTask: async (_hostPeer: string, agentId: string, message: A2aMessage) => {
    const { ws } = get();
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      throw new Error("未连接");
    }

    const requestId = nextRequestId++;
    const frame = {
      jsonrpc: "2.0",
      id: requestId,
      method: "tasks/create",
      params: {
        agentId,
        message,
      },
    };

    return new Promise<string>((resolve, reject) => {
      pendingRequests.set(requestId, { resolve, reject });
      ws.send(JSON.stringify(frame));

      setTimeout(() => {
        if (pendingRequests.has(requestId)) {
          pendingRequests.delete(requestId);
          reject(new Error("请求超时"));
        }
      }, 30000);
    });
  },

  sendTaskMessage: async (taskId: string, message: A2aMessage) => {
    const { ws } = get();
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      throw new Error("未连接");
    }

    const requestId = nextRequestId++;
    const frame = {
      jsonrpc: "2.0",
      id: requestId,
      method: "tasks/send",
      params: {
        taskId,
        message,
      },
    };

    return new Promise<void>((resolve, reject) => {
      pendingRequests.set(requestId, { resolve: () => resolve(), reject });
      ws.send(JSON.stringify(frame));

      setTimeout(() => {
        if (pendingRequests.has(requestId)) {
          pendingRequests.delete(requestId);
          reject(new Error("请求超时"));
        }
      }, 30000);
    });
  },

  cancelTask: async (taskId: string) => {
    const { ws } = get();
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      throw new Error("未连接");
    }

    const requestId = nextRequestId++;
    const frame = {
      jsonrpc: "2.0",
      id: requestId,
      method: "tasks/cancel",
      params: {
        taskId,
      },
    };

    return new Promise<void>((resolve, reject) => {
      pendingRequests.set(requestId, { resolve: () => resolve(), reject });
      ws.send(JSON.stringify(frame));

      setTimeout(() => {
        if (pendingRequests.has(requestId)) {
          pendingRequests.delete(requestId);
          reject(new Error("请求超时"));
        }
      }, 30000);
    });
  },

  getTask: (taskId: string) => {
    return get().tasks.get(taskId);
  },

  markRead: (agentKey: string) => {
    const { unreadByAgent } = get();
    if (unreadByAgent[agentKey]) {
      set({ unreadByAgent: { ...unreadByAgent, [agentKey]: 0 } });
    }
  },
}));

function handleFrame(
  frame: Record<string, unknown>,
  get: () => A2aState,
  set: (partial: Partial<A2aState>) => void,
) {
  // JSON-RPC 2.0 应答
  if (frame.id && typeof frame.id === "number") {
    const pending = pendingRequests.get(frame.id);
    if (pending) {
      pendingRequests.delete(frame.id);
      if (frame.error) {
        const errorObj = frame.error as { message?: string };
        pending.reject(new Error(errorObj.message || "请求失败"));
      } else {
        pending.resolve(frame.result);
      }
    }
    return;
  }

  // task 通知
  if (frame.method === "tasks/status") {
    const notice = frame.params as A2aTaskStatusNotice;
    const { tasks } = get();
    const task = tasks.get(notice.taskId);
    if (task) {
      const updated = { ...task, state: notice.state };
      const newTasks = new Map(tasks);
      newTasks.set(notice.taskId, updated);
      set({ tasks: newTasks });
    }
    return;
  }

  if (frame.method === "tasks/message") {
    const notice = frame.params as A2aTaskMessageNotice;
    const { tasks } = get();
    const task = tasks.get(notice.taskId);
    if (task) {
      // messageId 去重（流式多帧同 taskId）
      const existingIndex = task.messages.findIndex(
        (m) => m.messageId === notice.messageId
      );
      
      let newMessages: A2aMessage[];
      if (existingIndex >= 0) {
        // 同 messageId 的 text parts 按到达序拼接（增量语义）
        const existing = task.messages[existingIndex];
        const mergedParts = mergeParts(existing.parts, notice.message.parts);
        newMessages = [...task.messages];
        newMessages[existingIndex] = { ...existing, parts: mergedParts };
      } else {
        newMessages = [...task.messages, notice.message];
      }

      const updated = { ...task, messages: newMessages };
      const newTasks = new Map(tasks);
      newTasks.set(notice.taskId, updated);
      set({ tasks: newTasks });
    }
    return;
  }

  // tasks/get 快照（断线恢复权威终态）
  if (frame.method === "tasks/get" && frame.result) {
    const snapshot = frame.result as A2aTaskSnapshot;
    const { tasks } = get();
    const newTasks = new Map(tasks);
    newTasks.set(snapshot.taskId, {
      taskId: snapshot.taskId,
      agentId: snapshot.agentId,
      state: snapshot.state,
      messages: snapshot.messages,
    });
    set({ tasks: newTasks });
    return;
  }
}

function mergeParts(existing: A2aPart[], incoming: A2aPart[]): A2aPart[] {
  // 简单合并：将 incoming 的 text parts 拼接到 existing 的最后一个 text part
  const result = [...existing];
  // findLastIndex 兼容实现（TypeScript 目标可能不支持 ES2023）
  let lastTextIndex = -1;
  for (let i = result.length - 1; i >= 0; i--) {
    if (result[i].type === "text") {
      lastTextIndex = i;
      break;
    }
  }
  
  for (const part of incoming) {
    if (part.type === "text" && lastTextIndex >= 0) {
      const lastText = result[lastTextIndex] as A2aPart & { type: "text" };
      result[lastTextIndex] = {
        ...lastText,
        text: lastText.text + part.text,
      };
    } else {
      result.push(part);
    }
  }
  
  return result;
}