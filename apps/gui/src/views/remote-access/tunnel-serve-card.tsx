import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Locale } from "@/i18n";

import { toastError } from "@/components/feedback/toast";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ipc } from "@/lib/ipc";
import type { TunnelServeStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";

import { TunnelAllowTable } from "./tunnel-allow-table";
import { TunnelErrorBox } from "./tunnel-error-box";
import { TunnelShareGuide } from "./tunnel-share-guide";
import { matchErrorCode } from "./tunnel-flow";
import type { TunnelBannerInput } from "./tunnel-terminal-banner";
import { TargetPortField } from "./target-port-field";

interface Props {
  serve: TunnelServeStatus;
  onServeUpdate: (serve: TunnelServeStatus) => void;
  onError: (message: string) => void;
  onBanner: (banner: TunnelBannerInput) => void;
}

interface ServeOps {
  busy: boolean;
  lastError: string | null;
  addedAtByTarget: Record<string, number | null>;
  guideTarget: string | null;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

// 动作收敛为 hook：start 成功记录本会话加入时间并生成对端说明；终态上抛横幅。
function useServeOps(
  port: string,
  { onServeUpdate, onError, onBanner }: Props,
): ServeOps {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);
  const [addedAtByTarget, setAddedAtByTarget] = useState<Record<string, number | null>>({});
  const [guideTarget, setGuideTarget] = useState<string | null>(null);

  const start = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    const target = `127.0.0.1:${port.trim()}`;
    try {
      const report = await ipc.tunnelServeStart(target);
      setAddedAtByTarget((prev) => ({ ...prev, [target]: Date.now() }));
      setGuideTarget(target);
      onServeUpdate(report);
      onBanner({ tone: "success", target });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 被访开启失败", error);
      setLastError(message);
      onError(message);
      onBanner({ tone: "error", reason: message, code: matchErrorCode(message) });
      toastError(t("remoteAccess.serve.title"), { description: message });
    } finally {
      setBusy(false);
    }
  }, [onBanner, onError, onServeUpdate, port, t]);

  const stop = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const report = await ipc.tunnelServeStop();
      onServeUpdate(report);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 被访关闭失败", error);
      setLastError(message);
      onError(message);
      onBanner({ tone: "error", reason: message, code: matchErrorCode(message) });
      toastError(t("remoteAccess.serve.title"), { description: message });
    } finally {
      setBusy(false);
    }
  }, [onBanner, onError, onServeUpdate, t]);

  return { busy, lastError, addedAtByTarget, guideTarget, start, stop };
}

// tunnel 被访服务卡片（gui-contract §19.1）：端口输入 → tunnel_serve_start
// （白名单累积 + 开启受理）/ tunnel_serve_stop（关受理、白名单保留）。
// 白名单表格化 + 开放成功生成对端使用说明（gap-matrix §4.1 纯前端栏）。
export function TunnelServeCard(props: Props) {
  const { serve } = props;
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const selfPeerId = useNodeStore((s) => s.status?.peerId ?? null);
  const [port, setPort] = useState("");
  const { busy, lastError, addedAtByTarget, guideTarget, start, stop } =
    useServeOps(port, props);

  return (
    <Card className="col-span-12 lg:col-span-6" data-testid="tunnel-serve-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {t("remoteAccess.serve.title")}
          <Badge
            variant={serve.enabled ? "default" : "secondary"}
            data-testid="tunnel-serve-state"
          >
            {serve.enabled
              ? t("remoteAccess.serve.statusEnabled")
              : t("remoteAccess.serve.statusDisabled")}
          </Badge>
        </CardTitle>
        <CardDescription>{t("remoteAccess.serve.description")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <TargetPortField
          inputId="tunnel-serve-port"
          label={t("remoteAccess.serve.port")}
          placeholder={t("remoteAccess.serve.portPlaceholder")}
          hint={t("remoteAccess.serve.portHint")}
          value={port}
          onChange={setPort}
        />
        <div className="flex gap-2">
          <Button
            type="button"
            onClick={() => void start()}
            disabled={busy || port.trim() === ""}
            data-testid="tunnel-serve-start"
          >
            {busy ? t("remoteAccess.serve.starting") : t("remoteAccess.serve.start")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={() => void stop()}
            disabled={busy || !serve.enabled}
            data-testid="tunnel-serve-stop"
          >
            {busy ? t("remoteAccess.serve.stopping") : t("remoteAccess.serve.stop")}
          </Button>
        </div>
        <TunnelAllowTable
          allow={serve.allow}
          enabled={serve.enabled}
          addedAtByTarget={addedAtByTarget}
          locale={locale}
        />
        {guideTarget ? (
          <TunnelShareGuide selfPeerId={selfPeerId} target={guideTarget} />
        ) : null}
        {lastError && (
          <TunnelErrorBox message={lastError} testId="tunnel-serve-error" />
        )}
      </CardContent>
    </Card>
  );
}
