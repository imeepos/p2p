import { TriangleAlertIcon, WifiOffIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { useNodeStore } from "@/stores/node-store";

// 数据链路可信横幅（monitor P0）：引导失败给显式错误态与重试入口；
// 周期刷新连败给「数据可能已过期」警示。均为 store 状态派生，
// 恢复成功后自动消失，不会把最后一次数据当实时数据无声展示。
export function DataLinkBanner() {
  const { t } = useTranslation();
  const phase = useNodeStore((s) => s.bootstrapPhase);
  const bootstrapError = useNodeStore((s) => s.bootstrapError);
  const dataStale = useNodeStore((s) => s.dataStale);
  const lastRefreshError = useNodeStore((s) => s.lastRefreshError);
  const bootstrap = useNodeStore((s) => s.bootstrap);
  const refresh = useNodeStore((s) => s.refresh);

  if (phase === "error") {
    return (
      <div className="grid grid-cols-12 gap-4 px-6 pt-4">
        <div
          role="alert"
          className="border-destructive/40 bg-destructive/10 text-destructive col-span-12 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-md border px-4 py-2.5 text-sm"
        >
          <WifiOffIcon className="size-4 shrink-0" aria-hidden />
          <span className="font-medium">
            {t("dashboard.dataLink.bootstrapFailed")}
          </span>
          {bootstrapError && (
            <span className="text-muted-foreground min-w-0 truncate text-xs">
              {bootstrapError}
            </span>
          )}
          <span className="flex-1" />
          <AsyncButton
            size="sm"
            variant="outline"
            action={bootstrap}
            loadingLabel={t("dashboard.dataLink.retrying")}
          >
            {t("dashboard.dataLink.retry")}
          </AsyncButton>
        </div>
      </div>
    );
  }

  if (dataStale) {
    return (
      <div className="grid grid-cols-12 gap-4 px-6 pt-4">
        <div
          role="status"
          aria-live="polite"
          className="border-amber-500/40 bg-amber-500/10 text-amber-700 col-span-12 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-md border px-4 py-2.5 text-sm dark:text-amber-400"
        >
          <TriangleAlertIcon className="size-4 shrink-0" aria-hidden />
          <span className="font-medium">{t("dashboard.dataLink.stale")}</span>
          {lastRefreshError && (
            <span className="text-muted-foreground min-w-0 truncate text-xs">
              {lastRefreshError}
            </span>
          )}
          <span className="flex-1" />
          <AsyncButton
            size="sm"
            variant="outline"
            action={refresh}
            loadingLabel={t("dashboard.dataLink.refreshing")}
          >
            {t("dashboard.dataLink.refreshNow")}
          </AsyncButton>
        </div>
      </div>
    );
  }

  return null;
}
