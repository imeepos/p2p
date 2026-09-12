import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
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
import type { TunnelOpenReport, TunnelStatusReport } from "@/lib/ipc-types";
import { PageHeader } from "@/components/page/page-header";

import { GenericTunnelCard } from "./generic-tunnel-card";

type Phase = "idle" | "open" | "error";

const IDLE_STATUS: TunnelStatusReport = {
  active: false,
  localAddr: null,
  target: null,
  sessions: [],
  serve: { enabled: false, allow: [], activeSessions: 0 },
};

// 远程访问页（W-T3）：粘贴 A 机 dsh web 启动 URL + 被访节点 PeerId →
// Rust 侧绑 127.0.0.1 反代并经隧道转发 → 系统浏览器打开入口链接。
// 通用入口（§19.3-9）：target=127.0.0.1:<port> 字面量 + PeerId → tunnel_open。
// 三态：未开启 / 已开启 / 错误；关闭随应用退出自动收尾（契约 §7 无关闭命令）。
export function RemoteAccessView() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<TunnelStatusReport>(IDLE_STATUS);
  const [openUrl, setOpenUrl] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [lastError, setLastError] = useState<string | null>(null);
  const [url, setUrl] = useState("");
  const [peer, setPeer] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    ipc
      .tunnelStatus()
      .then((current) => {
        if (!alive) return;
        setStatus(current);
        // 错误态不回退：快照仅在没有更新的错误展示时生效。
        setPhase((prev) =>
          prev === "error" ? prev : current.active ? "open" : "idle",
        );
      })
      .catch((error) => console.error("[tunnel] 状态读取失败", error));
    const unlisten = ipc.onTunnelStatus((next) => {
      setStatus(next);
      setPhase(next.active ? "open" : "idle");
    });
    return () => {
      alive = false;
      void unlisten.then((off) => off());
    };
    // 挂载时订阅一次；开/关状态由命令返回值与事件驱动。
  }, []);

  const open = useCallback(async () => {
    setBusy(true);
    setLastError(null);
    try {
      const result = await ipc.tunnelOpenDsh(url.trim(), peer.trim());
      setOpenUrl(result.openUrl);
      setStatus((prev) => ({
        ...prev,
        active: true,
        localAddr: result.localAddr,
      }));
      setPhase("open");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[tunnel] 开启失败", error);
      setLastError(message);
      setPhase("error");
      toastError(message);
    } finally {
      setBusy(false);
    }
  }, [peer, url]);

  // 通用入口回调：成功/失败都汇入页面共享状态（状态卡与错误展示复用）。
  const openGeneric = useCallback((report: TunnelOpenReport) => {
    setOpenUrl(report.openUrl);
    setStatus((prev) => ({
      ...prev,
      active: true,
      localAddr: report.localAddr,
    }));
    setPhase("open");
  }, []);

  const onGenericError = useCallback((message: string) => {
    setLastError(message);
    setPhase("error");
    toastError(message);
  }, []);

  const phaseBadge =
    phase === "open"
      ? { label: t("remoteAccess.status.open"), tone: "default" as const }
      : phase === "error"
        ? { label: t("remoteAccess.status.error"), tone: "destructive" as const }
        : { label: t("remoteAccess.status.idle"), tone: "secondary" as const };

  return (
    <>
      <PageHeader
        titleKey="remoteAccess.title"
        descriptionKey="remoteAccess.description"
      />
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
            <Label htmlFor="tunnel-dsh-url">
              {t("remoteAccess.form.url")}
            </Label>
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
      <GenericTunnelCard onOpened={openGeneric} onError={onGenericError} />
      <Card className="col-span-12 lg:col-span-6">
        <CardHeader>
          <CardTitle>{t("remoteAccess.status.open")}</CardTitle>
          <CardDescription>{t("remoteAccess.status.reopenHint")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-2 text-sm">
          <Row label={t("remoteAccess.status.localAddr")} value={status.localAddr} />
          <Row label={t("remoteAccess.status.openUrl")} value={openUrl} />
          <Row label={t("remoteAccess.status.target")} value={status.target} />
          <Row
            label={t("remoteAccess.status.activeConns")}
            value={String(status.sessions.filter((x) => x.outcome === "open").length)}
          />
          {status.sessions
            .slice(-3)
            .reverse()
            .map((session) => (
              <Row
                key={session.sessionId}
                label={session.sessionId}
                value={`${session.bytesIn}/${session.bytesOut} ${session.outcome}`}
              />
            ))}
          <Button
            type="button"
            variant="secondary"
            disabled={!openUrl}
            onClick={() => {
              if (!openUrl) return;
              navigator.clipboard
                .writeText(openUrl)
                .then(() => toastSuccess(t("remoteAccess.status.copied")))
                .catch((error) => {
                  console.error("[tunnel] 复制入口链接失败", error);
                  toastError(t("remoteAccess.status.copyUrl"));
                });
            }}
          >
            {t("remoteAccess.status.copyUrl")}
          </Button>
        </CardContent>
      </Card>
    </>
  );
}

function Row({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="flex justify-between gap-4 border-b pb-1">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono break-all text-right">{value ?? "-"}</span>
    </div>
  );
}
