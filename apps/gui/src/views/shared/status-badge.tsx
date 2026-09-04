import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export type StatusTone = "success" | "danger" | "warning" | "neutral";

// 底色/描边走主题语义变量（--success/--destructive/--warning），文字用
// 固定色阶保证亮暗两态都过 AA（语义变量作小字文字色亮色下对比不足）。
const TONE_CLASS: Record<StatusTone, string> = {
  success: "border-success/30 bg-success/10 text-emerald-700 dark:text-emerald-300",
  danger: "border-destructive/30 bg-destructive/10 text-red-700 dark:text-red-300",
  warning: "border-warning/40 bg-warning/10 text-amber-800 dark:text-amber-300",
  neutral: "border-border bg-muted/60 text-muted-foreground",
};

const DOT_CLASS: Record<StatusTone, string> = {
  success: "bg-success",
  danger: "bg-destructive",
  warning: "bg-warning",
  neutral: "bg-muted-foreground/50",
};

interface StatusBadgeProps {
  tone: StatusTone;
  children: ReactNode;
  dot?: boolean;
  title?: string;
  className?: string;
}

// 状态徽章规范（IM-V1 跨页一致性）：success/danger/warning/neutral 四档，
// 统一 inline-flex 圆角小徽章形状；一处定义，各页状态语义复用。
export function StatusBadge({
  tone,
  children,
  dot = false,
  title,
  className,
}: StatusBadgeProps) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex w-fit shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md border px-2 py-0.5 text-xs font-medium",
        TONE_CLASS[tone],
        className,
      )}
    >
      {dot ? (
        <span
          aria-hidden
          className={cn("size-1.5 rounded-full", DOT_CLASS[tone])}
        />
      ) : null}
      {children}
    </span>
  );
}
