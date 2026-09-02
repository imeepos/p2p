import {
  ArrowLeftRightIcon,
  CheckIcon,
  WaypointsIcon,
  XIcon,
  ZapIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import type { DialHopJson, DialHopKind } from "@/lib/ipc-types";
import { HOP_KEY } from "@/views/monitor/hop-labels";
import { cn } from "@/lib/utils";

const HOP_ICON: Record<DialHopKind, typeof ArrowLeftRightIcon> = {
  direct: ArrowLeftRightIcon,
  punch: ZapIcon,
  relay: WaypointsIcon,
};

interface HopTimelineProps {
  hops: DialHopJson[];
  className?: string;
}

// 降级链逐跳时间线：direct/punch/relay 图标 + 成败色 + detail。
export function HopTimeline({ hops, className }: HopTimelineProps) {
  const { t } = useTranslation();

  return (
    <ol className={cn("flex flex-col", className)}>
      {hops.map((hop, index) => {
        const Icon = HOP_ICON[hop.hop];
        return (
          <li key={index} className="relative flex gap-3 pb-4 last:pb-0">
            {index < hops.length - 1 && (
              <span
                className="bg-border absolute top-7 left-[11px] h-[calc(100%-1.25rem)] w-px"
                aria-hidden
              />
            )}
            <span
              className={cn(
                "z-10 flex size-6 shrink-0 items-center justify-center rounded-full border",
                hop.ok
                  ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                  : "border-destructive/40 bg-destructive/10 text-destructive",
              )}
            >
              <Icon className="size-3.5" aria-hidden />
            </span>
            <div className="min-w-0 flex-1 pt-0.5">
              <div className="flex items-center gap-1.5 text-sm font-medium">
                {t(HOP_KEY[hop.hop])}
                {hop.ok ? (
                  <CheckIcon className="size-3.5 text-emerald-600 dark:text-emerald-400" aria-hidden />
                ) : (
                  <XIcon className="text-destructive size-3.5" aria-hidden />
                )}
              </div>
              <p className="text-muted-foreground mt-0.5 break-words text-xs">
                {hop.detail}
              </p>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
