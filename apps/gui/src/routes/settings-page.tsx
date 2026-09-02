import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { StatCard } from "@/components/page/stat-card";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";

export function SettingsPage() {
  const { t } = useTranslation();
  const status = useNodeStore((s) => s.status);
  const [config, setConfig] = useState<GuiConfig | null>(null);

  useEffect(() => {
    let cancelled = false;
    ipc.configGet()
      .then((cfg) => {
        if (!cancelled) setConfig(cfg);
      })
      .catch((error) => {
        console.error("[settings] config_get 失败", error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const effective = status?.config ?? config;

  return (
    <>
      <PageHeader
        titleKey="settings.title"
        descriptionKey="settings.description"
      />
      <StatCard
        label={t("dashboard.cards.status")}
        value={
          status
            ? status.running
              ? t("common.state.running")
              : t("common.state.stopped")
            : t("common.state.unknown")
        }
      />
      <StatCard
        label={t("common.labels.ports")}
        value={effective ? String(effective.quicPort) + " / " + String(effective.tcpPort) : undefined}
        loading={effective === null}
      />
      <StatCard
        label="mDNS"
        value={effective ? String(effective.enableMdns) : undefined}
        loading={effective === null}
      />
      <StatCard
        label={t("common.labels.version")}
        value={"v" + __APP_VERSION__}
      />
      <div className="col-span-12">
        <Card>
          <CardHeader>
            <CardTitle>{t("settings.title")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-1 font-mono text-xs">
            <p className="text-muted-foreground font-sans text-sm">
              {t("settings.hint")}
            </p>
            {effective ? (
              <>
                <div>dataDir: {effective.dataDir}</div>
                <div>bootstrap: {effective.bootstrap.join(", ") || "-"}</div>
                <div>relayAddrs: {effective.relayAddrs.join(", ") || "-"}</div>
                <div>
                  advertisedAddrs: {effective.advertisedAddrs.join(", ") || "-"}
                </div>
                <div>
                  observation: {String(effective.observationPort ?? "-")}{" "}
                  {effective.observationAddrs.join(", ")}
                </div>
              </>
            ) : null}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
