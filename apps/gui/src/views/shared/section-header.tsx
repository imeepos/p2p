import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// 分区头色调：primary=主色（入群邀请/agents 图标 chip）、info=蓝（好友邀请），
// agents=紫（/agents 页节头）。新页接入优先复用本组件，禁另造节头。
export type SectionTone = "primary" | "info" | "agents";

const toneChipClass: Record<SectionTone, string> = {
  primary: "bg-primary",
  info: "bg-info",
  agents: "bg-violet-600",
};

interface SectionHeaderProps {
  icon: LucideIcon;
  title: string;
  tone?: SectionTone;
  /** 待处理/条目计数：>0 时显示红 pill（--wx-badge，与 rail 铃铛角标同源） */
  count?: number;
}

// 分区头：彩色圆角图标 chip + 标题 + 计数角标。消息中心（IMC3）原样升为
// 共享组件（A2A3 F8 泛化）：testid 前缀中性化为 section-count-<tone>。
export function SectionHeader({
  icon: Icon,
  title,
  tone = "primary",
  count,
}: SectionHeaderProps) {
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
          data-testid={"section-count-" + tone}
          className="bg-wx-badge inline-flex min-w-4 items-center justify-center rounded-full px-1.5 text-[10px] leading-4 font-medium text-white tabular-nums"
        >
          {count}
        </span>
      ) : null}
    </div>
  );
}
