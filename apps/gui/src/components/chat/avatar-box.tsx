import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// WX1 微信风格方形头像：列表/气泡/侧栏共用。src 缺省回退首字符 +
// seed 稳定取色（同 id 恒同色，无闪烁）；icon 优先级高于首字符（bot 条目）。
const AVATAR_TONES = [
  "bg-[#5b7cdb]",
  "bg-[#4ca0e0]",
  "bg-[#48b884]",
  "bg-[#e0a03c]",
  "bg-[#e06c5b]",
  "bg-[#9a6fe0]",
  "bg-[#48b0b8]",
  "bg-[#c274b8]",
] as const;

function avatarTone(seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) | 0;
  }
  return AVATAR_TONES[Math.abs(hash) % AVATAR_TONES.length];
}

const SIZE_CLASS = {
  sm: "size-8 rounded-[5px] text-xs",
  md: "size-10 rounded-md text-sm",
  lg: "size-9 rounded-md text-sm",
} as const;

export type AvatarBoxSize = keyof typeof SIZE_CLASS;

export interface AvatarBoxProps {
  /** 回退首字符与无障碍名；空串时回退 "P" */
  label: string;
  /** 头像图（data URL）；null/undefined 走字符回退 */
  src?: string | null;
  /** 稳定取色种子（peerId / groupId / endpointId） */
  seed: string;
  size?: AvatarBoxSize;
  icon?: LucideIcon;
  className?: string;
}

export function AvatarBox({
  label,
  src,
  seed,
  size = "md",
  icon: Icon,
  className,
}: AvatarBoxProps) {
  const initial = label.trim().charAt(0).toUpperCase() || "P";
  return (
    <span
      aria-hidden
      className={cn(
        "relative flex shrink-0 select-none items-center justify-center overflow-hidden font-medium text-white",
        src ? null : avatarTone(seed),
        SIZE_CLASS[size],
        className,
      )}
    >
      {src ? (
        <img src={src} alt="" className="size-full object-cover" />
      ) : Icon ? (
        <Icon className="size-1/2" aria-hidden />
      ) : (
        initial
      )}
    </span>
  );
}
