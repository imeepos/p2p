import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

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
import type { TunnelOpenReport } from "@/lib/ipc-types";

import { matchErrorCode } from "./tunnel-flow";
import { TunnelErrorBox } from "./tunnel-error-box";
import type { TunnelBannerInput } from "./tunnel-terminal-banner";

export type TunnelPhase = "idle" | "open" | "error";

interface Props {
  phase: TunnelPhase;
  onOpened: (report: TunnelOpenReport) => void;
  onError: (message: string) => void;
  onBanner: (banner: TunnelBannerInput) => void;
}

// DSH 专用入口卡片（§19.3-4）：粘贴 A 机 dsh web 启动 URL + 被访节点
// PeerId → tunnel_open_dsh；终态上抛页面横幅（成功含链接与 peer）。
export function DshOpenCard({ phase, onOpened, onError, onBanner }: Props) {
  const { t } = useTranslation();
  const [url, setUrl] = useState("");
  const [peer, setPeer] = useState("");
  const [busy, setBusy] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const open = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const result = await ipc.tunnelOpenDsh(url.trim(), peer.trim());
      onOpened(result);
      onBanner({ tone: "success", link: result.openUrl, peer: peer.trim() });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 开启失败", error);
      setLastError(message);
      onError(message);
      onBanner({ tone: "error", reason: message, code: matchErrorCode(message) });
    } finally {
      setBusy(false);
    }
  }, [onBanner, onOpened, onError, peer, url]);

  const phaseBadge =
    phase === "open"
      ? { label: t("remoteAccess.status.open"), tone: "default" as const }
      : phase === "error"
        ? { label: t("remoteAccess.status.error"), tone: "destructive" as const }
        : { label: t("remoteAccess.status.idle"), tone: "secondary" as const };

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {t("remoteAccess.title")}
          <Badge variant={phaseBadge.tone}>{phaseBadge.label}</Badge>
        </CardTitle>
        <CardDescription>{t("remoteAccess.form.urlHint")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="tunnel-dsh-url">{t("remoteAccess.form.url")}</Label>
          <Input
            id="tunnel-dsh-url"
            value={url}
            placeholder={t("remoteAccess.form.urlPlaceholder")}
            onChange={(e) => setUrl(e.target.value)}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="tunnel-peer">{t("remoteAccess.form.peer")}</Label>
          <Input
            id="tunnel-peer"
            value={peer}
            placeholder={t("remoteAccess.form.peerPlaceholder")}
            onChange={(e) => setPeer(e.target.value)}
          />
          <p className="text-muted-foreground text-xs">
            {t("remoteAccess.form.peerHint")}
          </p>
        </div>
        <Button type="button" onClick={() => void open()} disabled={busy}>
          {busy ? t("remoteAccess.form.opening") : t("remoteAccess.form.open")}
        </Button>
        {phase === "error" && lastError && (
          <TunnelErrorBox message={lastError} showNodeHint />
        )}
      </CardContent>
    </Card>
  );
}
