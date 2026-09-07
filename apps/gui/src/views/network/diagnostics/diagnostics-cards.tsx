import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { getRecentErrors, type FrontendErrorEntry, type FrontendErrorKind } from "@/lib/error-report";
import { formatDateTime } from "@/lib/format";
import { copyText } from "@/views/shared/clipboard";
import type { I18nKey } from "@/i18n/types";
import type { Locale } from "@/i18n";

// N15：kind 徽章人话化——内部枚举不直出。
const ERROR_KIND_KEY: Record<FrontendErrorKind, I18nKey> = {
  error: "diagnostics.errors.kind.error",
  unhandledrejection: "diagnostics.errors.kind.unhandledrejection",
  console: "diagnostics.errors.kind.console",
};

// 诊断页三卡（G-H 观测 + F27 降级）：非 Tauri 环境（浏览器 mock dev）没有
// 诊断 IPC 与本地日志，环境卡/日志尾卡显示「桌面端可用」说明性空态并停用
// 无效动作；前端错误缓冲属浏览器侧数据照常展示，原始堆栈折叠为「复制详情」。

export function EnvCard({
  logPath,
  desktop,
}: {
  logPath: string | null;
  desktop: boolean;
}) {
  const { t } = useTranslation();
  const copyPath = () => {
    if (!logPath) return;
    void copyText(logPath, {
      done: t("diagnostics.env.copied"),
      failed: t("common.copyFailed"),
    });
  };
  return (
    <Card className="col-span-12">
      <CardHeader>
        <CardTitle>{t("diagnostics.env.title")}</CardTitle>
        <CardDescription>{t("diagnostics.env.description")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm">
        {desktop ? (
          <>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground">{t("diagnostics.env.mode")}</span>
              {/* 诊断数据固定走真实 Tauri IPC（禁止 mock），运行环境恒为 Tauri 桥接 */}
              <Badge>{t("diagnostics.env.tauri")}</Badge>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-muted-foreground">{t("diagnostics.env.logPath")}</span>
              <code className="bg-muted rounded px-2 py-1 break-all">{logPath ?? "…"}</code>
              <Button variant="outline" size="sm" onClick={copyPath} disabled={!logPath}>
                {t("diagnostics.env.copy")}
              </Button>
            </div>
          </>
        ) : (
          <div className="flex flex-col gap-1" data-testid="diagnostics-desktop-only-env">
            <p className="text-foreground text-sm font-medium">
              {t("uxk.diagnostics.desktopOnlyTitle")}
            </p>
            <p className="text-muted-foreground text-xs">
              {t("uxk.diagnostics.desktopOnlyHint")}
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export function ErrorBufferCard({
  version,
  onRefresh,
  onClear,
}: {
  version: number;
  onRefresh: () => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  const entries = [...getRecentErrors()].reverse();
  return (
    <Card className="col-span-12" data-version={version}>
      <CardHeader>
        <CardTitle>{t("diagnostics.errors.title")}</CardTitle>
        <CardDescription>{t("diagnostics.errors.description")}</CardDescription>
        <CardAction className="flex gap-2">
          <Button variant="outline" size="sm" onClick={onRefresh}>
            {t("diagnostics.refresh")}
          </Button>
          <Button variant="outline" size="sm" onClick={onClear}>
            {t("diagnostics.clearAll")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        {entries.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("diagnostics.errors.empty")}</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {entries.map((entry, i) => (
              <ErrorRow key={entry.ts + String(i)} entry={entry} index={i} />
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

// F27：堆栈是开发者向信息，默认折叠进「复制详情」，正文只留人读消息。
function ErrorRow({ entry, index }: { entry: FrontendErrorEntry; index: number }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const tsMs = new Date(entry.ts).getTime();
  const copyDetails = () => {
    const details = entry.stack ? entry.message + "\n" + entry.stack : entry.message;
    void copyText(details, {
      done: t("uxk.errors.copyDetailsDone"),
      failed: t("common.copyFailed"),
    });
  };
  return (
    <li className="flex flex-col gap-1 text-sm">
      <div className="flex items-center gap-2">
        <Badge variant="outline">{t(ERROR_KIND_KEY[entry.kind])}</Badge>
        <span className="text-muted-foreground font-mono text-xs" title={entry.ts}>
          {formatDateTime(tsMs, locale)}
        </span>
      </div>
      <p className="break-all">{entry.message}</p>
      {entry.stack ? (
        <div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={copyDetails}
            data-testid={"diagnostics-error-copy-" + String(index)}
          >
            {t("uxk.errors.copyDetails")}
          </Button>
        </div>
      ) : null}
    </li>
  );
}

export function LogTailCard({
  tail,
  desktop,
  paused,
  onTogglePause,
  onRefresh,
}: {
  tail: string[];
  desktop: boolean;
  paused: boolean;
  onTogglePause: () => void;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const copyLog = () => {
    if (tail.length === 0) return;
    void copyText(tail.join("\n"), {
      done: t("common.copied"),
      failed: t("common.copyFailed"),
    });
  };
  return (
    <Card className="col-span-12">
      <CardHeader>
        <CardTitle>{t("diagnostics.tail.title")}</CardTitle>
        <CardDescription>{t("diagnostics.tail.description")}</CardDescription>
        <CardAction className="flex gap-2">
          <Button variant="outline" size="sm" onClick={onTogglePause} data-testid="diagnostics-tail-pause">
            {t(paused ? "common.actions.resume" : "common.actions.pause")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={copyLog}
            disabled={!desktop || tail.length === 0}
            data-testid="diagnostics-tail-copy"
          >
            {t("diagnostics.tail.copy")}
          </Button>
          <Button variant="outline" size="sm" onClick={onRefresh}>
            {t("diagnostics.refresh")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        {!desktop ? (
          <p className="text-muted-foreground text-sm" data-testid="diagnostics-desktop-only-tail">
            {t("uxk.diagnostics.desktopOnlyHint")}
          </p>
        ) : tail.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("diagnostics.tail.empty")}</p>
        ) : (
          <pre className="bg-muted max-h-64 overflow-auto rounded p-3 font-mono text-xs whitespace-pre-wrap">
            {tail.join("\n")}
          </pre>
        )}
      </CardContent>
    </Card>
  );
}
