import type {
  ChatKind,
  ChatMediaInput,
  GroupMessageJson,
  NodeEventJson,
} from "@/lib/ipc-types";
import { base64ByteSize, localId } from "@/lib/chat-local";

// 群会话端点本地纯逻辑：占位/合并/事件归并（记录形状与 1:1 不同，与
// chat-local 同型分立；不共享可变面，避免把群字段混进 1:1 缓存）。

// 去重 + tsMs 升序（旧→新，最新在末尾，与 1:1 气泡直排一致）。
export function mergeGroupMessages(
  existing: GroupMessageJson[],
  incoming: GroupMessageJson[],
): GroupMessageJson[] {
  const byId = new Map(existing.map((m) => [m.id, m]));
  for (const m of incoming) byId.set(m.id, m);
  return [...byId.values()].sort((a, b) => a.tsMs - b.tsMs);
}

// 乐观占位：senderId 取本机 PeerId（发送前置条件），返回后按占位 id 换真身。
export function placeholderGroupMessage(
  groupId: string,
  kind: ChatKind,
  text: string | null,
  selfPeerId: string,
  media?: ChatMediaInput,
  replyTo?: string | null,
): GroupMessageJson {
  return {
    id: localId(),
    groupId,
    senderId: selfPeerId,
    kind,
    tsMs: Date.now(),
    text,
    media: media
      ? {
          name: media.name,
          mime: media.mime,
          size: base64ByteSize(media.dataBase64),
          path: null,
        }
      : null,
    status: "pending",
    acks: [],
    replyTo: replyTo ?? null,
  };
}

// ---- 占位生命周期（store 侧最小可变面）----

interface GroupPendingMutable {
  messagesByGroup: Record<string, GroupMessageJson[]>;
  lastMessageByGroup: Record<string, GroupMessageJson | null>;
}

type SetFn<S> = (fn: (s: S) => Partial<S>) => void;

export function pushGroupPending<S extends GroupPendingMutable>(
  set: SetFn<S>,
  groupId: string,
  placeholder: GroupMessageJson,
): void {
  set((s) => ({
    messagesByGroup: {
      ...s.messagesByGroup,
      [groupId]: mergeGroupMessages(s.messagesByGroup[groupId] ?? [], [placeholder]),
    },
    lastMessageByGroup: { ...s.lastMessageByGroup, [groupId]: placeholder },
  }) as Partial<S>);
}

export function swapGroupPending<S extends GroupPendingMutable>(
  get: () => S,
  set: SetFn<S>,
  groupId: string,
  placeholderId: string,
  real: GroupMessageJson,
): void {
  const rest = (get().messagesByGroup[groupId] ?? []).filter(
    (m) => m.id !== placeholderId,
  );
  const next = mergeGroupMessages(rest, [real]);
  set((s) => ({
    messagesByGroup: { ...s.messagesByGroup, [groupId]: next },
    lastMessageByGroup: { ...s.lastMessageByGroup, [groupId]: real },
  }) as Partial<S>);
}

// 占位移除（取消/发送失败回滚）：列表摘除；摘要仍指向该占位则回退剩余末条。
export function retractGroupPending<S extends GroupPendingMutable>(
  set: SetFn<S>,
  groupId: string,
  placeholderId: string,
): void {
  set((s) => {
    const next = (s.messagesByGroup[groupId] ?? []).filter(
      (m) => m.id !== placeholderId,
    );
    const last = s.lastMessageByGroup[groupId];
    const summary =
      last && last.id === placeholderId ? (next[next.length - 1] ?? null) : last;
    return {
      messagesByGroup: { ...s.messagesByGroup, [groupId]: next },
      lastMessageByGroup: { ...s.lastMessageByGroup, [groupId]: summary ?? null },
    } as Partial<S>;
  });
}

// 群消息事件归并：入站消息去重追加并刷新摘要；ack 推进原地更新 acks/status。
// 返回 null = 事件不触及消息缓存（store 保持原引用）。
export function groupMessagesAfterEvent(
  messagesByGroup: Record<string, GroupMessageJson[]>,
  lastMessageByGroup: Record<string, GroupMessageJson | null>,
  event: NodeEventJson,
): {
  messagesByGroup: Record<string, GroupMessageJson[]>;
  lastMessageByGroup: Record<string, GroupMessageJson | null>;
} | null {
  if (event.type === "chat_group_message") {
    const groupId = event.groupId;
    const list = messagesByGroup[groupId] ?? [];
    if (list.some((m) => m.id === event.message.id)) return null;
    return {
      messagesByGroup: {
        ...messagesByGroup,
        [groupId]: mergeGroupMessages(list, [event.message]),
      },
      lastMessageByGroup: { ...lastMessageByGroup, [groupId]: event.message },
    };
  }
  if (event.type === "chat_group_status") {
    const list = messagesByGroup[event.groupId] ?? [];
    let touched = false;
    const next = list.map((m) => {
      if (m.id !== event.messageId) return m;
      touched = true;
      return { ...m, acks: [...event.acks], status: event.status };
    });
    if (!touched) return null;
    return {
      messagesByGroup: { ...messagesByGroup, [event.groupId]: next },
      lastMessageByGroup,
    };
  }
  return null;
}
