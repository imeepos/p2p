import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// 分区头色调：入群邀请=primary（微信绿）、好友邀请=info（蓝），与设计稿
// messages-page-pending.png 的类型图标色一致。
export type SectionTone = "primary" | "info";

const toneChipClass: Record<SectionTone, string> = {
  primary: "bg-primary",
  info: "bg-info",
};

interface MessageSectionHeaderProps {
  icon: LucideIcon;
  title: string;
  tone?: SectionTone;
  /** 待处理计数：>0 时显示红 pill（--wx-badge，与 rail 铃铛角标同源） */
  count?: number;
}

// 分区头：彩色圆角图标 chip + 标题 + 待处理计数角标（设计稿增量 1/2）。
export function MessageSectionHeader({
  icon: Icon,
  title,
  tone = "primary",
  count,
}: MessageSectionHeaderProps) {
  return (
    <div className="flex items-center gap-2">
      <span
        aria-hidden
        className={cn(
          "flex size-8 items-center justify-center rounded-lg",
          toneChipClass[tone],
        )}
      >
        <Icon className="size-4 text-white" />
      </span>
      <h2 className="text-sm font-semibold">{title}</h2>
      {typeof count === "number" && count > 0 ? (
        <span
          data-testid={"messages-section-count-" + tone}
          className="bg-wx-badge inline-flex min-w-4 items-center justify-center rounded-full px-1.5 text-[10px] leading-4 font-medium text-white tabular-nums"
        >
          {count}
        </span>
      ) : null}
    </div>
  );
}
