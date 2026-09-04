import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { useAcpStore } from "@/acp/acp-store";

/** 上下文占用条（ACP usage_update）：数值+比例，阈值对齐 RFD 建议分级 */
export function UsageBar() {
  const { t } = useTranslation();
  const usage = useAcpStore((s) =>
    s.activeSessionId ? s.interactions[s.activeSessionId]?.usage : undefined,
  );
  if (!usage || !usage.size || usage.size <= 0) return null;
  const used = usage.used ?? 0;
  const percent = Math.min(100, Math.round((used / usage.size) * 100));
  return (
    <div className="flex flex-col gap-1" data-testid="acp-usage-bar">
      <div className="text-muted-foreground flex items-center justify-between text-xs">
        <span>{t("acp.usage.title")}</span>
        <span data-testid="acp-usage-text">
          {t("acp.usage.detail", { used, size: usage.size, percent })}
        </span>
      </div>
      <div className="bg-muted h-1.5 w-full overflow-hidden rounded-full">
        <div
          className={cn("h-full rounded-full",
            percent >= 95 ? "bg-destructive" : percent >= 75 ? "bg-warning" : "bg-success",
          )}
          style={{ width: percent + "%" }}
          data-testid="acp-usage-fill"
        />
      </div>
    </div>
  );
}