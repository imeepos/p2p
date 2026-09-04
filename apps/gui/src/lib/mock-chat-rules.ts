import type { ChatKind, ChatMediaInput } from "./ipc-types";

// 契约 §12 chat 侧纯校验规则：与 mock-chat/src-tauri/p2p-chat 三方同口径。
// 从 mock-chat.ts 拆出（行数红线），仅数据与判定，无运行时依赖。
const PEER_ID_RE = /^[1-9A-HJ-NP-Za-km-z]{43,45}$/;
const ADDR_RE = /^(?:\d{1,3}\.){3}\d{1,3}\/[ut]\d{1,5}$/;
const BASE64_RE = /^[A-Za-z0-9+/]+={0,2}$/;

export const MAX_TEXT_CHARS = 2000;
export const MAX_MEDIA_BYTES = 64 * 1024 * 1024; // 与 chunked.rs MAX_MESSAGE_SIZE 一致
export const MAX_NICKNAME_CHARS = 64;
export const MAX_GROUP_CHARS = 32; // 与 p2p-chat friend::MAX_GROUP_CHARS 同口径
export const MAX_GROUP_NAME_CHARS = 64; // 群名 trim 后 1..=64（im-group-design §1）
export const MAX_GROUP_MEMBERS = 32; // 群成员上限，含 owner（im-group-design §1）

export function isValidPeerId(value: string): boolean {
  return PEER_ID_RE.test(value);
}

export function isValidTransportAddr(value: string): boolean {
  if (!ADDR_RE.test(value)) return false;
  const [host, tail] = value.split("/");
  const octets = host.split(".").map(Number);
  const port = Number(tail.slice(1));
  return octets.every((octet) => octet >= 0 && octet <= 255) && port >= 1 && port <= 65535;
}

// 设计 §5 MIME 白名单：kind 与 mime 不匹配一律 Err，不猜不降级。
const MIME_BY_KIND: Record<
  Exclude<ChatKind, "text" | "file">,
  ReadonlySet<string>
> = {
  image: new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]),
  audio: new Set([
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/m4a",
    "audio/mp4",
  ]),
  video: new Set(["video/mp4", "video/webm", "video/mov", "video/quicktime"]),
};

// 设计 §4：去路径分隔符/控制字符，仅保留 [A-Za-z0-9._-]，空则回退 attachment。
export function sanitizeName(name: string): string {
  const kept = name.replace(/[^A-Za-z0-9._-]/g, "");
  return kept.length > 0 ? kept : "attachment";
}

export function mediaPath(peer: string, messageId: string, name: string): string {
  return `<app-data>/chat/media/${peer}/${messageId}_${sanitizeName(name)}`;
}

// 群媒体落盘路径：media/ 复用，群子目录键 = groupId（im-group-design §4）。
export function groupMediaPath(
  groupId: string,
  messageId: string,
  name: string,
): string {
  return `<app-data>/chat/media/${groupId}/${messageId}_${sanitizeName(name)}`;
}

// 群名（≠ 好友分组名）：trim 后 1..=64 字符，字符数按 Array.from 计。
export function validateGroupChatName(raw: string): string | null {
  const trimmed = raw.trim();
  const len = Array.from(trimmed).length;
  if (len === 0) return "群名不能为空";
  if (len > MAX_GROUP_NAME_CHARS) {
    return `群名超过 ${MAX_GROUP_NAME_CHARS} 字符上限`;
  }
  return null;
}

export function base64ByteSize(dataBase64: string): number {
  const padding = dataBase64.endsWith("==") ? 2 : dataBase64.endsWith("=") ? 1 : 0;
  return Math.floor((dataBase64.length / 4) * 3) - padding;
}

function expectedKind(mime: string): ChatKind {
  for (const [kind, set] of Object.entries(MIME_BY_KIND)) {
    if (set.has(mime)) return kind as ChatKind;
  }
  return "file";
}

function validateMedia(kind: ChatKind, media: ChatMediaInput): string | null {
  const mime = media.mime.toLowerCase();
  if (expectedKind(mime) !== kind) {
    return `媒体 mime 与 kind 不匹配：${media.mime} 不能作为 ${kind} 发送`;
  }
  if (!BASE64_RE.test(media.dataBase64)) return "附件 base64 载荷非法";
  const size = base64ByteSize(media.dataBase64);
  if (size > MAX_MEDIA_BYTES) {
    return `附件超过单条消息上限（${size} > ${MAX_MEDIA_BYTES} 字节）`;
  }
  return null;
}

// 消息载荷校验（文本/媒体规则），不含会话对象前置校验——1:1 的好友簿校验与
// 群聊的成员在册校验各自包装本函数，保证两边口径不漂移。
export function validateMessagePayload(
  kind: ChatKind,
  text: string | undefined,
  media: ChatMediaInput | undefined,
): string | null {
  if (kind === "text") {
    const trimmed = (text ?? "").trim();
    if (trimmed.length === 0) return "文本消息不能为空";
    if (trimmed.length > MAX_TEXT_CHARS) {
      return `文本超过 ${MAX_TEXT_CHARS} 字符上限`;
    }
    return null;
  }
  if (!media) return `kind=${kind} 的消息必须携带 media`;
  return validateMedia(kind, media);
}

function validateSend(
  peer: string,
  kind: ChatKind,
  text: string | undefined,
  media: ChatMediaInput | undefined,
  isFriend: boolean,
): string | null {
  if (!isFriend) return `对方还不是好友：${peer}`;
  return validateMessagePayload(kind, text, media);
}

// 回复引用校验（IM-T46A 契约）：提供时须非空字符串；不校验存在性（离线引用允许）。
export function validateReplyTo(replyTo: string | null | undefined): string | null {
  if (replyTo == null) return null;
  if (replyTo.trim().length === 0) return `回复引用非法：${replyTo}`;
  return null;
}

// 分组名校验（IM-T43，与 p2p-chat friend::validate_group 三方同口径）：
// trim 后 1..=32 字符；空串/纯空白 = 合法（移出分组语义），返回 null。
// 字符数按 Array.from 计（与 Rust chars() 对齐，emoji 不双计）。
export function validateGroupName(raw: string | null | undefined): string | null {
  if (raw == null) return null;
  const trimmed = raw.trim();
  if (trimmed.length === 0) return null;
  if (Array.from(trimmed).length > MAX_GROUP_CHARS) {
    return `分组名超过 ${MAX_GROUP_CHARS} 字符上限`;
  }
  return null;
}

export { validateSend };
