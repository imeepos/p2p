import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { AsyncButton } from "@/components/feedback/async-button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { I18nKey } from "@/i18n/types";
import { ipc } from "@/lib/ipc";
import type { ServiceView } from "@/lib/ipc-types";
import { errorText } from "@/views/shared/form-flow";
import { SettingsGroup } from "./settings-row";

type LoadState = "loading" | "ready" | "failed";

// 展示名 i18n 映射：闭集 id → services.name.* 键（snake_case id 原样透传，
// 未登记 id 回落原文展示，清单数据源始终是 servicesList 返回值）。
const NAME_KEY_SUFFIX: Record<string, string> = {
  "serve.llm_share": "serveLlmShare",
  "serve.tunnel": "serveTunnel",
  "serve.a2a": "serveA2a",
  "serve.acp": "serveAcp",
  "net.rendezvous_register": "netRendezvousRegister",
  "net.relay": "netRelay",
  "net.observe": "netObserve",
  "serve.rendezvous_server": "serveRendezvousServer",
  "discovery.mdns": "discoveryMdns",
  "net.lan_only": "netLanOnly",
};

const KIND_KEYS: Record<ServiceView["kind"], { label: I18nKey; hint: I18nKey }> = {
  boolean: { label: "services.kindBoolean", hint: "services.kindBooleanHint" },
  explicit: { label: "services.kindExplicit", hint: "services.kindExplicitHint" },
  adopted: { label: "services.kindAdopted", hint: "services.kindAdoptedHint" },
};

function serviceName(service: ServiceView, t: (key: I18nKey) => string): string {
  const suffix = NAME_KEY_SUFFIX[service.serviceId];
  return suffix
    ? t(`services.name.${suffix}` as I18nKey)
    : service.serviceId;
}

// 服务总控卡（gui-contract §20）：闭集清单 + 开关翻转 + 重启提示条。
// enabled 为持久化生效值；requiresRestart=true（节点运行中）顶部提示
// 「重启节点后生效」，面板不做本地推导冒充运行态（§20.4-4）。
export function ServicesCard() {
  const { t } = useTranslation();
  const [services, setServices] = useState<ServiceView[]>([]);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());

  const load = useCallback(async () => {
    const { services: list } = await ipc.servicesList();
    setServices(list);
    setLoadState("ready");
  }, []);

  // 首次加载失败：错误态可见不白屏（重试按钮复用 load，失败走按钮 onError）。
  // 挂载拉取走 effect 内联 IIFE（react-hooks/set-state-in-effect 合规形态，
  // offer-panel/use-gui-config 先例）；cancelled 守卫防卸载后回落错误态。
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const { services: list } = await ipc.servicesList();
        if (!cancelled) {
          setServices(list);
          setLoadState("ready");
        }
      } catch (error) {
        console.error("[services] services_list 失败", error);
        if (!cancelled) setLoadState("failed");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = useCallback(
    async (service: ServiceView, next: boolean) => {
      const snapshot = services;
      setServices((prev) =>
        prev.map((row) =>
          row.serviceId === service.serviceId ? { ...row, enabled: next } : row,
        ),
      );
      setPending((prev) => new Set(prev).add(service.serviceId));
      try {
        const report = await ipc.servicesSetEnabled(service.serviceId, next);
        setServices((prev) =>
          prev.map((row) =>
            row.serviceId === report.serviceId
              ? { ...row, enabled: report.enabled, requiresRestart: report.requiresRestart }
              : row,
          ),
        );
        toastSuccess(t("services.saveSuccess"));
      } catch (error) {
        console.error("[services] services_set_enabled 失败", error);
        setServices(snapshot);
        toastError(t("services.saveFailed"), { description: errorText(error) });
      } finally {
        setPending((prev) => {
          const next2 = new Set(prev);
          next2.delete(service.serviceId);
          return next2;
        });
      }
    },
    [services, t],
  );

  const restartPending = services.some((row) => row.requiresRestart);

  return (
    <SettingsGroup
      title={t("services.title")}
      description={t("services.description")}
    >
      {loadState === "failed" ? (
        <div className="flex flex-col items-start gap-2 py-3" data-testid="services-load-failed">
          <p className="text-destructive text-sm" role="alert">
            {t("services.loadFailed")}
          </p>
          <AsyncButton
            type="button"
            size="sm"
            variant="outline"
            action={load}
            loadingLabel={t("common.actions.refreshing")}
            onError={(error) => {
              console.error("[services] services_list 重试失败", error);
              toastError(t("services.loadFailed"), {
                description: errorText(error),
              });
            }}
          >
            {t("common.actions.refresh")}
          </AsyncButton>
        </div>
      ) : null}
      {loadState === "ready" && restartPending ? (
        <div
          role="status"
          data-testid="services-restart-hint"
          className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200"
        >
          {t("services.restartHint")}
        </div>
      ) : null}
      {loadState === "ready" ? (
        <Table containerClassName="mt-1">
          <TableHeader>
            <TableRow>
              <TableHead>{t("services.columnService")}</TableHead>
              <TableHead>{t("services.columnKind")}</TableHead>
              <TableHead>{t("services.columnState")}</TableHead>
              <TableHead className="text-right">
                {t("services.columnAction")}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {services.map((service) => {
              const kind = KIND_KEYS[service.kind] ?? null;
              const name = serviceName(service, t);
              return (
                <TableRow key={service.serviceId} data-testid={`service-row-${service.serviceId}`}>
                  <TableCell>
                    <div className="font-medium">{name}</div>
                    <div className="text-muted-foreground font-mono text-xs">
                      {service.serviceId}
                    </div>
                  </TableCell>
                  <TableCell>
                    {kind ? (
                      <Badge
                        variant="secondary"
                        title={t(kind.hint)}
                        data-testid={`service-kind-${service.serviceId}`}
                      >
                        {t(kind.label)}
                      </Badge>
                    ) : (
                      service.kind
                    )}
                  </TableCell>
                  <TableCell>
                    {service.enabled
                      ? t("services.stateEnabled")
                      : t("services.stateDisabled")}
                  </TableCell>
                  <TableCell className="text-right">
                    <Switch
                      aria-label={name}
                      data-testid={`service-switch-${service.serviceId}`}
                      checked={service.enabled}
                      disabled={pending.has(service.serviceId)}
                      onCheckedChange={(next) => void toggle(service, next)}
                    />
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      ) : null}
    </SettingsGroup>
  );
}
