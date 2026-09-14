import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { I18nKey } from "@/i18n/types";
import type { TunnelStatusReport } from "@/lib/ipc-types";
import { copyText } from "@/views/shared/clipboard";

import { deriveWsAddr } from "./tunnel-flow";

function Row({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="flex justify-between gap-4 border-b pb-1">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono break-all text-right">{value ?? "-"}</span>
    </div>
  );
}

// 访侧成功态卡：local_addr / 入口链接 / ws 派生地址（gap-matrix §3(a)
// 残余缺口——反代透传 ws 升级，仅缺提示）+ 近 3 条会话审计行。
export function TunnelStatusCard({
  status,
  openUrl,
}: {
  status: TunnelStatusReport;
  openUrl: string | null;
}) {
  const { t } = useTranslation();
  const wsAddr = deriveWsAddr(openUrl, status.localAddr);

  const copyAs = (text: string, doneKey: I18nKey) => {
    void copyText(text, {
      done: t(doneKey),
      failed: t("remoteAccess.tunnel.copyFailed"),
    });
  };

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("remoteAccess.status.open")}</CardTitle>
        <CardDescription>{t("remoteAccess.status.reopenHint")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <Row label={t("remoteAccess.status.localAddr")} value={status.localAddr} />
        <Row label={t("remoteAccess.status.openUrl")} value={openUrl} />
        {wsAddr ? (
          <Row label={t("remoteAccess.tunnel.wsAddr")} value={wsAddr} />
        ) : null}
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
        {wsAddr ? (
          <p className="text-muted-foreground text-xs">
            {t("remoteAccess.tunnel.wsHint")}
          </p>
        ) : null}
        <div className="flex gap-2">
          <Button
            type="button"
            variant="secondary"
            disabled={!openUrl}
            data-testid="tunnel-copy-url"
            onClick={() => openUrl && copyAs(openUrl, "remoteAccess.status.copied")}
          >
            {t("remoteAccess.status.copyUrl")}
          </Button>
          {wsAddr ? (
            <Button
              type="button"
              variant="secondary"
              data-testid="tunnel-copy-ws"
              onClick={() => copyAs(wsAddr, "remoteAccess.tunnel.wsCopied")}
            >
              {t("remoteAccess.tunnel.copyWs")}
            </Button>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
