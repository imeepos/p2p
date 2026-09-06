import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { useNodeStore } from "@/stores/node-store";

// UX1 自动启动的可感知呈现（AppLayout 挂载，全页面可见）：starting 走中性
// 状态条；failed 走显式错误条 + 可反复触发的重试入口，禁止静默失败或永挂
// 骨架；成功后 status.running 翻转，横幅自然消失。
export function AutoStartNotice() {
  const { t } = useTranslation();
  const phase = useNodeStore((s) => s.autoStartPhase);
  const error = useNodeStore((s) => s.autoStartError);
  const retryAutoStart = useNodeStore((s) => s.retryAutoStart);

  if (phase === "starting") {
    return (
      <div className="grid grid-cols-12 gap-4 px-6 pt-4">
        <div
          role="status"
          aria-live="polite"
          className="text-muted-foreground col-span-12 flex items-center gap-x-3 rounded-md border px-4 py-2.5 text-sm"
        >
          <span
            className="motion-safe:animate-pulse bg-muted-foreground size-1.5 shrink-0 rounded-full"
            aria-hidden
          />
          <span className="font-medium">{t("node.autoStart.starting")}</span>
        </div>
      </div>
    );
  }

  if (phase === "failed") {
    return (
      <div className="grid grid-cols-12 gap-4 px-6 pt-4">
        <div
          role="alert"
          className="border-destructive/40 bg-destructive/10 text-destructive col-span-12 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-md border px-4 py-2.5 text-sm"
        >
          <span className="font-medium">{t("node.autoStart.failed")}</span>
          {error && (
            <span className="text-muted-foreground min-w-0 truncate text-xs">
              {error}
            </span>
          )}
          <span className="flex-1" />
          <AsyncButton
            size="sm"
            variant="outline"
            action={retryAutoStart}
            loadingLabel={t("common.state.starting")}
          >
            {t("node.autoStart.retry")}
          </AsyncButton>
        </div>
      </div>
    );
  }

  return null;
}
