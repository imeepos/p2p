import type { GroupJson, GroupMessageJson, NodeEventJson } from "./ipc-types";
import { uuid } from "./mock-chat";

// 群聊 mock 共享内存态与快照工具（行数红线拆分）：roster 操作面在
// mock-group-roster.ts，发送/历史/媒体与组装在 mock-group-chat.ts。

export const ACK_DELAY_MS = 120;
export const REPLY_DELAY_MS = 400;
export const HISTORY_DEFAULT_LIMIT = 50;
export const HISTORY_MAX_LIMIT = 100;

export interface MockGroupState {
  groups: Map<string, GroupJson>;
  history: Map<string, GroupMessageJson[]>; // 每群时间升序追加
}

export const groupState: MockGroupState = {
  groups: new Map(),
  history: new Map(),
};

// mock 运行时接线依赖：读 mock-ipc 节点态（本机 PeerId/连接），发群事件。
export interface MockGroupChatDeps {
  emit(event: NodeEventJson): void;
  selfPeerId(): string;
  isConnected(peer: string): boolean;
  isFriend(peerId: string): boolean;
}

// 场景注入运行时：mock-group-inject 与测试经此驱动入站群消息与外部群播种；
// 不改变 IpcBackend 契约面。
export interface MockGroupChatRuntime {
  emit(event: NodeEventJson): void;
  newId(): string;
  appendMessage(message: GroupMessageJson): GroupMessageJson;
  seedGroup(group: GroupJson): GroupJson;
}

let activeRuntime: MockGroupChatRuntime | null = null;

// 未初始化时显式抛错（可观测信号），不静默。
export function activeMockGroupChatRuntime(): MockGroupChatRuntime {
  if (!activeRuntime) {
    throw new Error("mock group 后端未初始化：先调用 createMockGroupChatBackend");
  }
  return activeRuntime;
}

export function bindMockGroupRuntime(runtime: MockGroupChatRuntime): void {
  activeRuntime = runtime;
}

export function snapshotGroup(group: GroupJson): GroupJson {
  return { ...group, members: [...group.members] };
}

export function snapshotGroupMessage(message: GroupMessageJson): GroupMessageJson {
  return {
    ...message,
    media: message.media ? { ...message.media } : null,
    acks: [...message.acks],
  };
}

export function appendGroupMessage(message: GroupMessageJson): void {
  const log = groupState.history.get(message.groupId) ?? [];
  log.push(message);
  groupState.history.set(message.groupId, log);
}

export function requireGroup(groupId: string): GroupJson {
  const group = groupState.groups.get(groupId);
  if (!group) throw new Error(`群不存在：${groupId}`);
  return group;
}

export function requireOwner(group: GroupJson, self: string): void {
  if (group.owner !== self) throw new Error("仅群主可执行该操作");
}

export function touchRoster(group: GroupJson): void {
  group.rev += 1;
  group.tsMs = Date.now();
}

// roster 变更回执（契约 chat_group_state）：名单/rev/state 任一变更即发。
export function emitRoster(
  emit: (event: NodeEventJson) => void,
  group: GroupJson,
): void {
  emit({ type: "chat_group_state", group: snapshotGroup(group) });
}

export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export { uuid as newGroupMessageId };
