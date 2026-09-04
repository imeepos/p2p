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

// 空态规范（IM-V1 跨页一致性）：max-w 限宽居中、size-12 圆底图标、
// 主操作居中；对端/发现/聊天等页共用此组件保证形状一致。
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
        "text-muted-foreground mx-auto flex w-full max-w-md flex-col items-center justify-center gap-1.5 rounded-md border border-dashed p-8 text-center text-sm",
        className,
      )}
    >
      <span className="bg-muted mb-1 flex size-12 items-center justify-center rounded-full">
        <Icon className="size-6" aria-hidden />
      </span>
      <p className="text-foreground font-medium">{title}</p>
      {description ? <p className="max-w-80 text-xs">{description}</p> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
