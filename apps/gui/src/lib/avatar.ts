// 头像文件读取与压缩：居中裁方缩放至 128×128 PNG data URL。
// canvas 依赖浏览器环境，单测只覆盖纯几何函数与错误路径。
import { isValidAvatarDataUrl } from "./profile-rules";

export const AVATAR_EDGE = 128;
// 原图大小闸门：压缩前先拒超大文件，避免无谓的解码开销。
export const AVATAR_FILE_MAX_BYTES = 5 * 1024 * 1024;

export type AvatarErrorCode = "avatarTooLarge" | "avatarInvalid";

export class AvatarFileError extends Error {
  readonly code: AvatarErrorCode;

  constructor(code: AvatarErrorCode) {
    super(code);
    this.name = "AvatarFileError";
    this.code = code;
  }
}

export interface CoverCrop {
  sx: number;
  sy: number;
  sw: number;
  sh: number;
}

// 居中裁方几何（纯函数）：从 w×h 源图取最大居中正方形区域。
export function coverCrop(w: number, h: number): CoverCrop {
  const side = Math.min(w, h);
  return { sx: (w - side) / 2, sy: (h - side) / 2, sw: side, sh: side };
}

// 读取图片文件并压缩为 128×128 PNG data URL；超大/不可解码抛 AvatarFileError。
export async function fileToAvatarDataUrl(file: File): Promise<string> {
  if (file.size > AVATAR_FILE_MAX_BYTES) {
    throw new AvatarFileError("avatarTooLarge");
  }
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new AvatarFileError("avatarInvalid");
  }
  try {
    const canvas = document.createElement("canvas");
    canvas.width = AVATAR_EDGE;
    canvas.height = AVATAR_EDGE;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new AvatarFileError("avatarInvalid");
    const { sx, sy, sw, sh } = coverCrop(bitmap.width, bitmap.height);
    ctx.drawImage(bitmap, sx, sy, sw, sh, 0, 0, AVATAR_EDGE, AVATAR_EDGE);
    const dataUrl = canvas.toDataURL("image/png");
    if (!isValidAvatarDataUrl(dataUrl)) {
      throw new AvatarFileError("avatarTooLarge");
    }
    return dataUrl;
  } finally {
    bitmap.close();
  }
}
