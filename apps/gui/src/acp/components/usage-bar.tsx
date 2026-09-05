import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { useAcpStore } from "@/acp/acp-store";

/** 警示阈值：占用超过 80% 切警示色并文字提示，95% 升级危急（RFD 分级建议） */
const USAGE_WARN_PERCENT = 80;
const USAGE_CRIT_PERCENT = 95;

/** 上下文占用条（ACP usage_update）：数值+比例+分级警示提示 */
export function UsageBar() {
  const { t } = useTranslation();
  const usage = useAcpStore((s) =>
    s.activeSessionId ? s.interactions[s.activeSessionId]?.usage : undefined,
  );
  if (!usage || !usage.size || usage.size <= 0) return null;
  const used = usage.used ?? 0;
  const percent = Math.min(100, Math.round((used / usage.size) * 100));
  const critical = percent >= USAGE_CRIT_PERCENT;
  const warn = percent >= USAGE_WARN_PERCENT;
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
          className={cn(
            "h-full rounded-full",
            critical ? "bg-destructive" : warn ? "bg-warning" : "bg-success",
          )}
          style={{ width: percent + "%" }}
          data-testid="acp-usage-fill"
        />
      </div>
      {warn ? (
        <p
          className={cn("text-xs", critical ? "text-destructive" : "text-warning")}
          role="status"
          data-testid="acp-usage-hint"
        >
          {t(critical ? "acp.usage.critical" : "acp.usage.warning")}
        </p>
      ) : null}
    </div>
  );
}
