// T35 聊天边界测试共用夹具：确定性 PeerId / 契约 JSON 构造器 / 超限 File 桩。
// 只做纯数据构造，不引入 store 与 ipc 运行时，lib 单测与组件测试可共用。
import type {
  ChatFriendJson,
  ChatMediaJson,
  ChatMessageJson,
  ChatMessageStatus,
  ChatSendReport,
} from "@/lib/ipc-types";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

// 确定性合法 PeerId：4 字符前缀 + 40 个 base58 字符（契约要求 43-45 位）。
export function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

export function friendJson(peer: string, nickname = "好友"): ChatFriendJson {
  return { peerId: peer, nickname, addrs: [], note: null };
}

interface MessageOverrides {
  sender?: "me" | "them";
  status?: ChatMessageStatus;
  tsMs?: number;
}

export function textMessage(
  id: string,
  peer: string,
  text: string,
  overrides: MessageOverrides = {},
): ChatMessageJson {
  return {
    id,
    peer,
    sender: overrides.sender ?? "me",
    kind: "text",
    tsMs: overrides.tsMs ?? (Number(id.replace(/\D/g, "")) || 1),
    text,
    media: null,
    status: overrides.status ?? "delivered",
  };
}

export function chatMedia(
  name: string,
  mime: string,
  size = 1,
  path: string | null = null,
): ChatMediaJson {
  return { name, mime, size, path };
}

// kind 显式传入：白名单判定属于被测实现（chat-media），夹具不复制规则。
export function mediaMessage(
  id: string,
  peer: string,
  kind: ChatMessageJson["kind"],
  media: ChatMediaJson,
  overrides: MessageOverrides = {},
): ChatMessageJson {
  return {
    id,
    peer,
    sender: overrides.sender ?? "me",
    kind,
    tsMs: overrides.tsMs ?? 2,
    text: null,
    media,
    status: overrides.status ?? "pending",
  };
}

export function sendReport(message: ChatMessageJson, delivered = true): ChatSendReport {
  return { message, delivered };
}

// jsdom File 桩：构造器无法伪造 size，用原型桩表达 64MiB+1 的超限文件。
export function oversizedFile(name = "huge.bin", type = "application/octet-stream"): File {
  const stub = Object.create(File.prototype) as File;
  Object.defineProperties(stub, {
    name: { value: name },
    type: { value: type },
    size: { value: 64 * 1024 * 1024 + 1 },
  });
  return stub;
}
