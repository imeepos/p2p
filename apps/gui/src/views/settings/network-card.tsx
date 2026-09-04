import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useNodeStore } from "@/stores/node-store";
import type { SettingsFormValues } from "./config-schema";
import { ErrorText } from "@/views/shared/error-text";

// 从监听地址提取实际生效端口：QUIC 记 /端口 或 /u端口，TCP 固定 /t端口。
function effectivePort(listenAddrs: string[], tcp: boolean): number | null {
  const pattern = tcp ? /\/t(\d+)$/ : /\/(?:u)?(\d+)$/;
  for (const addr of listenAddrs) {
    const matched = addr.match(pattern);
    if (matched) return Number(matched[1]);
  }
  return null;
}

interface PortFieldProps {
  name: "quicPort" | "tcpPort";
  htmlId: string;
  label: string;
  effective: number | null;
}

// 端口输入：0（随机）不裸显，输入框置空并展示随机端口语义；节点运行中
// 就近展示当前实际生效端口。
function PortField({ name, htmlId, label, effective }: PortFieldProps) {
  const { t } = useTranslation();
  const {
    control,
    setValue,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  const value = useWatch({ control, name });
  const isRandom = value === 0 || value == null;

  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor={htmlId}>{label}</Label>
      <Input
        id={htmlId}
        type="number"
        inputMode="numeric"
        min={0}
        max={65535}
        placeholder={t("settings.network.randomPortPlaceholder")}
        value={isRandom ? "" : String(value)}
        onChange={(event) => {
          const parsed = Number(event.target.value);
          const next =
            event.target.value === "" || Number.isNaN(parsed) ? 0 : parsed;
          setValue(name, next, { shouldDirty: true });
        }}
      />
      <ErrorText code={errors[name]?.message} />
      {isRandom ? (
        <p className="text-muted-foreground text-xs">
          {t("settings.network.randomPortHint")}
        </p>
      ) : null}
      {effective !== null ? (
        <p className="text-muted-foreground text-xs">
          {t("settings.network.effectivePort", { port: effective })}
        </p>
      ) : null}
    </div>
  );
}

// 网络卡：quic/tcp 端口（0 = 随机）与 mDNS 开关。
export function NetworkCard() {
  const { t } = useTranslation();
  const { setValue } = useFormContext<SettingsFormValues>();
  const enableMdns = useWatch({ name: "enableMdns" });
  const status = useNodeStore((s) => s.status);
  const listenAddrs = status?.running ? status.listenAddrs : [];

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.network")}</CardTitle>
        <CardDescription>{t("settings.network.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <PortField
          name="quicPort"
          htmlId="settings-quic-port"
          label={t("settings.network.quicPort")}
          effective={effectivePort(listenAddrs, false)}
        />
        <PortField
          name="tcpPort"
          htmlId="settings-tcp-port"
          label={t("settings.network.tcpPort")}
          effective={effectivePort(listenAddrs, true)}
        />
        <div className="flex items-center justify-between gap-4">
          <div className="flex min-w-0 flex-1 flex-col">
            <Label htmlFor="settings-mdns">{t("settings.network.mdns")}</Label>
            {/* 长描述限宽 + 行高统一（IM-V2 S4）：与短标签行同一节奏 */}
            <p className="text-muted-foreground max-w-sm text-xs leading-5">
              {t("settings.network.mdnsHint")}
            </p>
          </div>
          <Switch
            id="settings-mdns"
            className="shrink-0"
            checked={enableMdns}
            onCheckedChange={(next) =>
              setValue("enableMdns", next, { shouldDirty: true })
            }
          />
        </div>
      </CardContent>
    </Card>
  );
}
