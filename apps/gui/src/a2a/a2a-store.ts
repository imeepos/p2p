// A2A 任务管理 store（docs/design/a2a-over-p2p-design.md §5.2/§8.3）。
// task 相 JSON-RPC 2.0 语义：tasks/create|send|cancel；通知 tasks/status + tasks/message。
// 1 task = 1 流（Q10）：每个任务独占一条 proto=a2a 泵连接（task-socket），
// 通知按 taskId 路由入簿；连接面凭据由会话挂载时 configureChannel 注入。

import { create } from "zustand";

import type {
  A2aTask,
  A2aMessage,
  A2aTaskStatusNotice,
  A2aTaskMessageNotice,
  A2aPart
} from "./task-types";
import { TaskSocket, taskChannelUrl, type TaskNotice } from "./task-socket";

interface ChannelConfig {
  wsUrl: string;
  token: string;
}

interface A2aState {
  /** taskId → task 快照。 */
  tasks: Map<string, A2aTask>;
  /** agentKey → 未读计数。 */
  unreadByAgent: Record<string, number>;
  /** 连接面凭据就绪（console WS connected 后由会话注入）。 */
  channelReady: boolean;
  /** 最近错误（可读，UI 上浮，禁静默）。 */
  lastError: string | null;

  configureChannel: (wsUrl: string, token: string) => void;
  createTask: (hostPeer: string, agentId: string, message: A2aMessage) => Promise<string>;
  sendTaskMessage: (taskId: string, message: A2aMessage) => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  getTask: (taskId: string) => A2aTask | undefined;
  markRead: (agentKey: string) => void;
}

/** 每任务连接（非渲染状态，不进 store，同 agents-store 卡片通道先例）。 */
const sockets = new Map<string, TaskSocket>();
let channel: ChannelConfig | null = null;
/** 任务入簿前到达的通知（create 应答与 upsert 之间的时序窗口），入簿后重放。 */
const pendingNotices = new Map<string, TaskNotice[]>();
const PENDING_NOTICE_CAP = 64;

function requireChannel(): ChannelConfig {
  if (!channel) throw new Error("任务通道未配置（console 未连接）");
  return channel;
}

function upsertTask(
  get: () => A2aState,
  set: (partial: Partial<A2aState>) => void,
  task: A2aTask,
): void {
  const tasks = new Map(get().tasks);
  tasks.set(task.taskId, task);
  set({ tasks });
}

function applyNotice(notice: TaskNotice, task: A2aTask): A2aTask {
  if ("state" in notice) {
    return { ...task, state: (notice as A2aTaskStatusNotice).state };
  }
  const message = notice as A2aTaskMessageNotice;
  const existingIndex = task.messages.findIndex((m) => m.messageId === message.messageId);
  if (existingIndex >= 0) {
    // 同 messageId 的 text parts 按到达序拼接（增量语义）
    const existing = task.messages[existingIndex];
    const mergedParts = mergeParts(existing.parts, message.message.parts);
    const nextMessages = [...task.messages];
    nextMessages[existingIndex] = { ...existing, parts: mergedParts };
    return { ...task, messages: nextMessages };
  }
  const stamped: A2aMessage = {
    ...message.message,
    messageId: message.message.messageId ?? message.messageId,
    receivedAtMs: Date.now(),
  };
  return { ...task, messages: [...task.messages, stamped] };
}

function routeNotice(
  get: () => A2aState,
  set: (partial: Partial<A2aState>) => void,
  notice: TaskNotice,
): void {
  const task = get().tasks.get(notice.taskId);
  if (!task) {
    // 任务尚未入簿（create 应答先于 upsert）：缓冲限容，入簿后重放，不静默丢弃
    const list = pendingNotices.get(notice.taskId) ?? [];
    if (list.length < PENDING_NOTICE_CAP) list.push(notice);
    pendingNotices.set(notice.taskId, list);
    return;
  }
  upsertTask(get, set, applyNotice(notice, task));
}

function drainNotices(
  get: () => A2aState,
  set: (partial: Partial<A2aState>) => void,
  taskId: string,
): void {
  const list = pendingNotices.get(taskId);
  if (!list?.length) return;
  pendingNotices.delete(taskId);
  let task = get().tasks.get(taskId);
  for (const notice of list) {
    if (!task) return;
    task = applyNotice(notice, task);
  }
  if (task) upsertTask(get, set, task);
}

export const useA2aStore = create<A2aState>((set, get) => {
  const openTaskSocket = (hostPeer: string): TaskSocket => {
    const config = requireChannel();
    return TaskSocket.open(taskChannelUrl(config.wsUrl, config.token, hostPeer), {
      onNotice: (notice) => routeNotice(get, set, notice),
      onClosed: (reason) => {
        if (reason === "closed by client") return;
        set({ lastError: reason });
        console.warn("[a2a] 任务流断开", reason);
      },
    });
  };

  return {
    tasks: new Map(),
    unreadByAgent: {},
    channelReady: false,
    lastError: null,

    configureChannel: (wsUrl, token) => {
      channel = { wsUrl, token };
      set({ channelReady: true, lastError: null });
    },

    createTask: async (hostPeer, agentId, message) => {
      const socket = openTaskSocket(hostPeer);
      try {
        await socket.awaitOpen();
        const result = (await socket.request("tasks/create", { agentId, message })) as
          | { taskId?: unknown }
          | null;
        const taskId = typeof result?.taskId === "string" ? result.taskId : "";
        if (!taskId) throw new Error("tasks/create 应答缺少 taskId");
        sockets.set(taskId, socket);
        upsertTask(get, set, { taskId, agentId, state: "submitted", messages: [message] });
        drainNotices(get, set, taskId);
        return taskId;
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        set({ lastError: reason });
        socket.close();
        throw error;
      }
    },

    sendTaskMessage: async (taskId, message) => {
      const socket = sockets.get(taskId);
      if (!socket) throw new Error("任务连接不存在（会话已重开）");
      try {
        await socket.request("tasks/send", { taskId, message });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        set({ lastError: reason });
        throw error;
      }
    },

    cancelTask: async (taskId) => {
      const socket = sockets.get(taskId);
      if (!socket) throw new Error("任务连接不存在（会话已重开）");
      try {
        await socket.request("tasks/cancel", { taskId });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        set({ lastError: reason });
        throw error;
      }
    },

    getTask: (taskId) => get().tasks.get(taskId),

    markRead: (agentKey) => {
      const { unreadByAgent } = get();
      if (unreadByAgent[agentKey]) {
        set({ unreadByAgent: { ...unreadByAgent, [agentKey]: 0 } });
      }
    },
  };
});

function mergeParts(existing: A2aPart[], incoming: A2aPart[]): A2aPart[] {
  // 简单合并：将 incoming 的 text parts 拼接到 existing 的最后一个 text part
  const result = [...existing];
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
