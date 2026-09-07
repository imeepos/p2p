import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { PageHeader } from "@/components/page/page-header";
import { clearErrorBufferAndQueue } from "@/lib/error-report";
import { diag } from "@/lib/ipc";
import { isTauriRuntime } from "@/lib/tauri-env";
import { errorText } from "@/views/shared/form-flow";

import { EnvCard, ErrorBufferCard, LogTailCard } from "./diagnostics-cards";

const TAIL_LINES = 50;
const AUTO_REFRESH_MS = 5000;

// 诊断页（G-H 观测）：前端错误缓冲 + 日志文件路径 + 持久化尾部，感知通道的人工视图。
// 失败反馈分档：轮询/首载失败静默（console 告警，否则每 5s toast 刷屏），
// 手动刷新失败才 toast（i18n 标题 + 错误详情，原始错误串进详情不当正文直出）。
// F27：非 Tauri 环境（浏览器 mock dev）没有诊断 IPC，直接进入桌面端说明性
// 空态并停掉轮询（否则每 5s toast 刷屏）；前端错误缓冲为浏览器侧数据照常。
export function DiagnosticsView() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const desktop = isTauriRuntime();
  const [logPath, setLogPath] = useState<string | null>(null);
  const [tail, setTail] = useState<string[]>([]);
  const [version, setVersion] = useState(0);

  // load 只做异步取数（.then 内 setState），供 effect 与定时器直接调用；
  // silent 分档：轮询/首载静默（仅 console），手动刷新失败才 toast 打扰。
  const load = useCallback(
    (silent: boolean) => {
      if (!desktop) return;
      diag
        .logPath()
        .then(setLogPath)
        .catch((err) => {
          console.error("[diagnostics] log_path 读取失败", err);
          if (!silent) {
            toastError(t("diagnostics.loadPathFailed"), {
              description: errorText(err),
              context: "diagnostics.log_path",
            });
          }
        });
      diag.logTail(TAIL_LINES).then(setTail).catch((err) => {
        console.error("[diagnostics] log_tail 读取失败", err);
        if (!silent) {
          toastError(t("diagnostics.loadTailFailed"), {
            description: errorText(err),
            context: "diagnostics.log_tail",
          });
        }
      });
    },
    [desktop, t],
  );

  // 手动刷新：失败 toast（load 内 silent=false）+ 成功轻反馈。
  const refresh = useCallback(() => {
    setVersion((v) => v + 1);
    load(false);
    toastSuccess(t("diagnostics.refreshed"));
  }, [load, t]);

  // 一键清理：清错误缓冲 + 删持久化日志文件，删除动作先过确认弹框。
  // 非桌面端没有日志文件可清，只清前端错误缓冲。
  const clearAll = useCallback(async () => {
    const ok = await confirm({
      title: t("diagnostics.clearConfirm.title"),
      description: t("diagnostics.clearConfirm.description"),
      confirmText: t("diagnostics.clearConfirm.confirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    try {
      clearErrorBufferAndQueue();
      if (desktop) await diag.logClear();
      setTail([]);
      setVersion((v) => v + 1);
      toastSuccess(t("diagnostics.cleared"));
    } catch (err) {
      console.error("[diagnostics] 清理诊断数据失败", err);
      toastError(t("diagnostics.clearFailed"), {
        description: errorText(err),
        context: "diagnostics.log_clear",
      });
    }
  }, [confirm, desktop, t]);

  // 首载与 5s 轮询同档：静默取数，失败只留 console 信号。
  useEffect(() => {
    if (!desktop) return;
    load(true);
    const timer = window.setInterval(() => load(true), AUTO_REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [desktop, load]);

  return (
    <>
      <PageHeader titleKey="diagnostics.title" descriptionKey="diagnostics.description" />
      <EnvCard logPath={logPath} desktop={desktop} />
      <ErrorBufferCard version={version} onRefresh={refresh} onClear={clearAll} />
      <LogTailCard tail={tail} desktop={desktop} onRefresh={refresh} />
    </>
  );
}
