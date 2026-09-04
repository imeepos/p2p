import type { ChatMediaInput, ChatMessageJson, ChatKind } from "@/lib/ipc-types";

// 端点本地占位/合并纯逻辑：与 store 分离便于行数红线与独立单测。
let localSeq = 0;

export function localId(): string {
  localSeq += 1;
  return `local-${Date.now()}-${localSeq}`;
}

// 合并页数据：去重 + 按 tsMs 升序（旧→新，最新在末尾便于气泡直排）。
export function mergeMessages(
  existing: ChatMessageJson[],
  incoming: ChatMessageJson[],
): ChatMessageJson[] {
  const byId = new Map(existing.map((m) => [m.id, m]));
  for (const m of incoming) byId.set(m.id, m);
  return [...byId.values()].sort((a, b) => a.tsMs - b.tsMs);
}

// base64 载荷字节估算（去 padding），占位气泡展示用，与后端解码一致。
export function base64ByteSize(dataBase64: string): number {
  const padding = dataBase64.endsWith("==") ? 2 : dataBase64.endsWith("=") ? 1 : 0;
  return Math.floor((dataBase64.length / 4) * 3) - padding;
}

export function placeholderMessage(
  peer: string,
  kind: ChatKind,
  text: string | null,
  media?: ChatMediaInput,
  replyTo?: string | null,
): ChatMessageJson {
  return {
    id: localId(),
    peer,
    sender: "me",
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
    replyTo: replyTo ?? null,
  };
}

// 占位被取消后（removeLocal 先执行）chatSend 返回不应复活本地条目标。
export function replaceLocal(
  list: ChatMessageJson[],
  localId: string,
  real: ChatMessageJson,
): ChatMessageJson[] {
  if (!list.some((m) => m.id === localId)) return list;
  return mergeMessages(list.filter((m) => m.id !== localId), [real]);
}

export function removeLocal(
  list: ChatMessageJson[],
  localMessageId: string,
): ChatMessageJson[] {
  return list.filter((m) => m.id !== localMessageId);
}

// ---- 占位生命周期（自 chat-store.ts 迁入，行数红线）----

// store 侧最小可变面：占位操作只触及这两个缓存。
interface PendingMutable {
  messagesByPeer: Record<string, ChatMessageJson[]>;
  lastMessageByPeer: Record<string, ChatMessageJson | null>;
}

type SetFn<S> = (fn: (s: S) => Partial<S>) => void;

export function pushPending<S extends PendingMutable>(
  set: SetFn<S>,
  peer: string,
  placeholder: ChatMessageJson,
): void {
  set((s) => ({
    messagesByPeer: {
      ...s.messagesByPeer,
      [peer]: mergeMessages(s.messagesByPeer[peer] ?? [], [placeholder]),
    },
    lastMessageByPeer: { ...s.lastMessageByPeer, [peer]: placeholder },
  }) as Partial<S>);
}

export function swapPending<S extends PendingMutable>(
  get: () => S,
  set: SetFn<S>,
  peer: string,
  placeholderId: string,
  real: ChatMessageJson,
): void {
  const next = replaceLocal(get().messagesByPeer[peer] ?? [], placeholderId, real);
  set((s) => ({
    messagesByPeer: { ...s.messagesByPeer, [peer]: next },
    lastMessageByPeer: { ...s.lastMessageByPeer, [peer]: real },
  }) as Partial<S>);
}

// 占位移除统一回滚（IM-T48 裁决项）：列表摘除占位；若摘要仍指向该占位
// （期间无新事件写入）则回退为剩余最后一条，无剩余清空；期间新事件的
// 摘要保持不动。媒体发送失败后列表不再残留占位文件名。
export function retractPending<S extends PendingMutable>(
  set: SetFn<S>,
  peer: string,
  placeholderId: string,
): void {
  set((s) => {
    const next = removeLocal(s.messagesByPeer[peer] ?? [], placeholderId);
    const last = s.lastMessageByPeer[peer];
    const summary =
      last && last.id === placeholderId ? (next[next.length - 1] ?? null) : last;
    return {
      messagesByPeer: { ...s.messagesByPeer, [peer]: next },
      lastMessageByPeer: { ...s.lastMessageByPeer, [peer]: summary ?? null },
    } as Partial<S>;
  });
}
