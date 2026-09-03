// 节点资料字段限制（契约 v6 §11）：与后端 profile.rs 同规则，常量与校验唯一来源。

export const NAME_MAX_CHARS = 64;
export const DESCRIPTION_MAX_CHARS = 280;
export const AVATAR_MAX_LEN = 200_000;

export const AVATAR_MIME_PREFIXES = [
  "data:image/png;base64,",
  "data:image/jpeg;base64,",
  "data:image/webp;base64,",
] as const;

const BASE64_PAYLOAD_RE = /^[A-Za-z0-9+/=]*$/;

// 与后端 validate_avatar 同规则：长度上限、MIME 白名单、base64 载荷字符集。
export function isValidAvatarDataUrl(url: string): boolean {
  if (url.length > AVATAR_MAX_LEN) return false;
  const prefix = AVATAR_MIME_PREFIXES.find((p) => url.startsWith(p));
  if (!prefix) return false;
  return BASE64_PAYLOAD_RE.test(url.slice(prefix.length));
}

export interface NodeProfileInput {
  name: string;
  description: string;
  avatar: string | null;
}

// 保存前镜像后端校验（契约 §11）；返回 null 表示通过，否则为可读中文错误。
export function validateNodeProfile(profile: NodeProfileInput): string | null {
  if ([...profile.name.trim()].length > NAME_MAX_CHARS) {
    return "节点名称过长，上限 " + NAME_MAX_CHARS + " 字符";
  }
  if ([...profile.description].length > DESCRIPTION_MAX_CHARS) {
    return "节点描述过长，上限 " + DESCRIPTION_MAX_CHARS + " 字符";
  }
  if (profile.avatar !== null && !isValidAvatarDataUrl(profile.avatar)) {
    return "头像格式不支持或数据过大，仅允许 PNG/JPEG/WebP 的 base64 data URL";
  }
  return null;
}
