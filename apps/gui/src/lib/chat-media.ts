import type { ChatKind, ChatMediaInput } from "@/lib/ipc-types";

// 设计 §5 附件规则：单条 ≤64MiB；kind 校验与 MIME 白名单与 mock/real 后端一致。
export const MAX_MEDIA_BYTES = 64 * 1024 * 1024;

const MIME_BY_KIND: Record<Exclude<ChatKind, "text" | "file">, readonly string[]> = {
  image: ["image/png", "image/jpeg", "image/gif", "image/webp"],
  audio: ["audio/mpeg", "audio/wav", "audio/ogg", "audio/m4a", "audio/mp4"],
  video: ["video/mp4", "video/webm", "video/mov", "video/quicktime"],
};

// 扩展名兜底映射：浏览器 File.type 常为空，按扩展名推断 mime 再落白名单。
const MIME_BY_EXT: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
  m4a: "audio/m4a",
  mp4: "video/mp4",
  webm: "video/webm",
  mov: "video/quicktime",
  quicktime: "video/quicktime",
};

function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

// 供 bulk 复用：mime 命中白名单取对应 kind，其余一律 file（不猜不降级，对齐 §5）。
export function kindForMime(mime: string): ChatKind {
  const normalized = mime.toLowerCase();
  for (const [kind, list] of Object.entries(MIME_BY_KIND)) {
    if (list.includes(normalized)) return kind as ChatKind;
  }
  return "file";
}

// 由文件名与浏览器 mime 推断展示 kind；mime 缺失时按扩展名补齐。
export function inferKind(name: string, fileType: string): ChatKind {
  const type = fileType.toLowerCase();
  if (type) return kindForMime(type);
  const mime = MIME_BY_EXT[extOf(name)];
  return mime ? kindForMime(mime) : "file";
}

export function resolveMime(name: string, fileType: string): string {
  return fileType.toLowerCase() || MIME_BY_EXT[extOf(name)] || "application/octet-stream";
}

// FileReader 读 base64 载荷（去掉 dataURL 前缀），失败落可读错误。
export function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = String(reader.result ?? "");
      const comma = dataUrl.indexOf(",");
      resolve(comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl);
    };
    reader.onerror = () => reject(new Error("readFile 失败"));
    reader.readAsDataURL(file);
  });
}

// 组装契约 ChatMediaInput；超限返回可读错误（组件 toast 呈现）。
export async function fileToChatMedia(file: File): Promise<ChatMediaInput> {
  if (file.size > MAX_MEDIA_BYTES) {
    throw new Error(`附件超过单条消息上限（${file.size} > ${MAX_MEDIA_BYTES} 字节）`);
  }
  return {
    name: file.name,
    mime: resolveMime(file.name, file.type),
    dataBase64: await readFileAsBase64(file),
  };
}
