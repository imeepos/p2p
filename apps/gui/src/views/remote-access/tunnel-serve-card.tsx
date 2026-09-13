import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ipc } from "@/lib/ipc";
import type { TunnelServeStatus } from "@/lib/ipc-types";

interface Props {
  serve: TunnelServeStatus;
  onServeUpdate: (serve: TunnelServeStatus) => void;
  onError: (message: string) => void;
}

// tunnel 被访服务卡片（gui-contract §19.1）：端口输入 → tunnel_serve_start
// （白名单累积 + 开启受理）/ tunnel_serve_stop（关受理、白名单保留）。
// target 固定拼 127.0.0.1:<port> 字面量；校验在命令层（先校验后动作），
// 错误进卡片错误盒 + toast，状态展示沿用 tunnel_status.serve 快照。
export function TunnelServeCard({ serve, onServeUpdate, onError }: Props) {
  const { t } = useTranslation();
  const [port, setPort] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const start = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const report = await ipc.tunnelServeStart(`127.0.0.1:${port.trim()}`);
      onServeUpdate(report);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 被访开启失败", error);
      setLastError(message);
      onError(message);
      toastError(t("remoteAccess.serve.title"), { description: message });
    } finally {
      setBusy(false);
    }
  }, [onError, onServeUpdate, port, t]);

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
      toastError(t("remoteAccess.serve.title"), { description: message });
    } finally {
      setBusy(false);
    }
  }, [onError, onServeUpdate, t]);

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
        <div className="space-y-2">
          <Label htmlFor="tunnel-serve-port">
            {t("remoteAccess.serve.port")}
          </Label>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground font-mono text-sm">
              127.0.0.1:
            </span>
            <Input
              id="tunnel-serve-port"
              inputMode="numeric"
              value={port}
              placeholder={t("remoteAccess.serve.portPlaceholder")}
              onChange={(e) => setPort(e.target.value)}
              className="flex-1"
            />
          </div>
          <p className="text-muted-foreground text-xs">
            {t("remoteAccess.serve.portHint")}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            type="button"
            onClick={() => void start()}
            disabled={busy || port.trim() === ""}
            data-testid="tunnel-serve-start"
          >
            {busy
              ? t("remoteAccess.serve.starting")
              : t("remoteAccess.serve.start")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            onClick={() => void stop()}
            disabled={busy || !serve.enabled}
            data-testid="tunnel-serve-stop"
          >
            {busy
              ? t("remoteAccess.serve.stopping")
              : t("remoteAccess.serve.stop")}
          </Button>
        </div>
        <div className="space-y-1 text-sm">
          <div className="flex justify-between gap-4 border-b pb-1">
            <span className="text-muted-foreground">
              {t("remoteAccess.serve.allowlist")}
            </span>
            <span className="font-mono break-all text-right">
              {serve.allow.length > 0
                ? serve.allow.join(", ")
                : t("remoteAccess.serve.emptyAllow")}
            </span>
          </div>
          <div className="flex justify-between gap-4 border-b pb-1">
            <span className="text-muted-foreground">
              {t("remoteAccess.serve.activeSessions")}
            </span>
            <span className="font-mono text-right">
              {serve.activeSessions}
            </span>
          </div>
        </div>
        {lastError && (
          <div
            role="alert"
            className="text-destructive space-y-1 rounded-md border border-red-300 p-3 text-sm"
          >
            <p className="font-medium">
              {t("remoteAccess.error.lastError")}
            </p>
            <p data-testid="tunnel-serve-error">{lastError}</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
