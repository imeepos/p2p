import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { PageHeader } from "@/components/page/page-header";
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
import { getRecentErrors, clearRecentErrors } from "@/lib/error-report";
import { diag } from "@/lib/ipc";
import { copyText } from "@/views/shared/clipboard";

const TAIL_LINES = 50;
const AUTO_REFRESH_MS = 5000;

// 诊断页（G-H 观测）：前端错误缓冲 + 日志文件路径 + 持久化尾部，感知通道的人工视图。
export function DiagnosticsView() {
  const [logPath, setLogPath] = useState<string | null>(null);
  const [tail, setTail] = useState<string[]>([]);
  const [version, setVersion] = useState(0);

  // load 只做异步取数（.then 内 setState），供 effect 与定时器直接调用。
  const load = useCallback(() => {
    diag.logPath().then(setLogPath).catch((err) => toastError(String(err)));
    diag
      .logTail(TAIL_LINES)
      .then(setTail)
      .catch((err) => toastError(String(err)));
  }, []);

  const refresh = useCallback(() => {
    setVersion((v) => v + 1);
    load();
  }, [load]);

  useEffect(() => {
    load();
    const timer = window.setInterval(load, AUTO_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [load]);

  return (
    <>
      <PageHeader titleKey="diagnostics.title" descriptionKey="diagnostics.description" />
      <EnvCard logPath={logPath} />
      <ErrorBufferCard version={version} onRefresh={refresh} />
      <LogTailCard tail={tail} onRefresh={refresh} />
    </>
  );
}

function EnvCard({ logPath }: { logPath: string | null }) {
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
      </CardContent>
    </Card>
  );
}

function ErrorBufferCard({ version, onRefresh }: { version: number; onRefresh: () => void }) {
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
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              clearRecentErrors();
              onRefresh();
              toastSuccess(t("diagnostics.errors.cleared"));
            }}
          >
            {t("diagnostics.errors.clear")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        {entries.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("diagnostics.errors.empty")}</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {entries.map((entry, i) => (
              <li key={entry.ts + String(i)} className="flex flex-col gap-1 text-sm">
                <div className="flex items-center gap-2">
                  <Badge variant="outline">{entry.kind}</Badge>
                  <span className="text-muted-foreground font-mono text-xs">{entry.ts}</span>
                </div>
                <p className="break-all">{entry.message}</p>
                {entry.stack && (
                  <pre className="bg-muted max-h-32 overflow-auto rounded p-2 text-xs whitespace-pre-wrap">
                    {entry.stack}
                  </pre>
                )}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function LogTailCard({ tail, onRefresh }: { tail: string[]; onRefresh: () => void }) {
  const { t } = useTranslation();
  return (
    <Card className="col-span-12">
      <CardHeader>
        <CardTitle>{t("diagnostics.tail.title")}</CardTitle>
        <CardDescription>{t("diagnostics.tail.description")}</CardDescription>
        <CardAction>
          <Button variant="outline" size="sm" onClick={onRefresh}>
            {t("diagnostics.refresh")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        {tail.length === 0 ? (
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