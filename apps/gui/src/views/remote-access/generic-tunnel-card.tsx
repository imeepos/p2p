import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

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
import type { TunnelOpenReport } from "@/lib/ipc-types";

interface Props {
  onOpened: (report: TunnelOpenReport) => void;
  onError: (message: string) => void;
}

// 通用服务开隧道卡片（gui-contract §19.3-9）：端口 + 被访节点 PeerId →
// tunnel_open（target 固定拼为 127.0.0.1:<port> 字面量，服务端校验）。
// 与 DSH 专用入口共享单会话（stop-旧-start-新）；成功态由状态卡展示
// local_addr 与可复制链接；错误态复用页面错误文案（role=alert 盒）。
export function GenericTunnelCard({ onOpened, onError }: Props) {
  const { t } = useTranslation();
  const [port, setPort] = useState("");
  const [peer, setPeer] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const open = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const report = await ipc.tunnelOpen(
        `127.0.0.1:${port.trim()}`,
        peer.trim(),
      );
      onOpened(report);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 通用开启失败", error);
      setLastError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  }, [onError, onOpened, peer, port]);

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("remoteAccess.generic.title")}</CardTitle>
        <CardDescription>
          {t("remoteAccess.generic.description")}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="tunnel-generic-target">
            {t("remoteAccess.generic.port")}
          </Label>
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground font-mono text-sm">
              127.0.0.1:
            </span>
            <Input
              id="tunnel-generic-target"
              inputMode="numeric"
              value={port}
              placeholder={t("remoteAccess.generic.portPlaceholder")}
              onChange={(e) => setPort(e.target.value)}
              className="flex-1"
            />
          </div>
          <p className="text-muted-foreground text-xs">
            {t("remoteAccess.generic.portHint")}
          </p>
        </div>
        <div className="space-y-2">
          <Label htmlFor="tunnel-generic-peer">
            {t("remoteAccess.generic.peer")}
          </Label>
          <Input
            id="tunnel-generic-peer"
            value={peer}
            placeholder={t("remoteAccess.generic.peerPlaceholder")}
            onChange={(e) => setPeer(e.target.value)}
          />
        </div>
        <Button
          type="button"
          onClick={() => void open()}
          disabled={busy || port.trim() === "" || peer.trim() === ""}
        >
          {busy
            ? t("remoteAccess.generic.opening")
            : t("remoteAccess.generic.open")}
        </Button>
        {lastError && (
          <div
            role="alert"
            className="text-destructive space-y-1 rounded-md border border-red-300 p-3 text-sm"
          >
            <p className="font-medium">{t("remoteAccess.error.lastError")}</p>
            <p>{lastError}</p>
            <p className="text-muted-foreground">
              {t("remoteAccess.error.nodeOfflineHint")}
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
