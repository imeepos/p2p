import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}

// 插画式空态：图标 + 文案 + 可选引导操作，替代裸文本"暂无"。
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        "text-muted-foreground flex flex-col items-center justify-center gap-1.5 rounded-md border border-dashed p-8 text-center text-sm",
        className,
      )}
    >
      <Icon className="mb-1 size-6" aria-hidden />
      <p className="text-foreground font-medium">{title}</p>
      {description ? <p className="max-w-96 text-xs">{description}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
