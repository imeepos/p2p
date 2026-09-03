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
