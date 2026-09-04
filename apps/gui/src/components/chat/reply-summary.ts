import type { ChatKind, ChatMessageJson } from "@/lib/ipc-types";

// 引用摘要截断上限：超长文本截断加省略号，气泡/预览均不爆炸。
export const REPLY_SUMMARY_MAX_CHARS = 80;

// 五类型引用摘要（契约 §12.3 + IM-T46B）：文本截断；图片/音频/视频仅类型词条；
// 文件显示文件名。
export function replySummaryOf(message: ChatMessageJson): string | null {
  if (message.kind === "text") {
    const text = message.text ?? "";
    if (text.length <= REPLY_SUMMARY_MAX_CHARS) return text;
    return text.slice(0, REPLY_SUMMARY_MAX_CHARS) + "…";
  }
  if (message.kind === "file") return message.media?.name ?? null;
  // image/audio/video：类型标识即摘要，不另出文件名行
  return null;
}

const KIND_KEYS = {
  text: "chat.reply.kindText",
  image: "chat.reply.kindImage",
  audio: "chat.reply.kindAudio",
  video: "chat.reply.kindVideo",
  file: "chat.reply.kindFile",
} as const;

export function replyKindKey(kind: ChatKind): (typeof KIND_KEYS)[ChatKind] {
  return KIND_KEYS[kind];
}
