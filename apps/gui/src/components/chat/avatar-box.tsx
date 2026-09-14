import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// WX1 微信风格方形头像：列表/气泡/侧栏共用。src 缺省回退首字符 +
// seed 稳定取色（同 id 恒同色，无闪烁）；icon 优先级高于首字符（bot 条目）。
// uix-spec 丑点 d：色板整体降饱和（oklch 彩度 ≤0.05），与灰白基调和解，
// 仍按 hue 八档可分辨
const AVATAR_TONES = [
  "bg-[oklch(0.68_0.05_250)]",
  "bg-[oklch(0.70_0.05_200)]",
  "bg-[oklch(0.70_0.05_155)]",
  "bg-[oklch(0.73_0.05_90)]",
  "bg-[oklch(0.69_0.05_45)]",
  "bg-[oklch(0.69_0.05_330)]",
  "bg-[oklch(0.68_0.045_285)]",
  "bg-[oklch(0.71_0.04_15)]",
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
