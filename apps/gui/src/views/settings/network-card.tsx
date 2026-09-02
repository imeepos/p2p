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
import type { GuiConfig } from "@/lib/ipc-types";
import { ErrorText } from "@/views/shared/error-text";

// 网络卡：quic/tcp 端口（0 = 随机）与 mDNS 开关。
export function NetworkCard() {
  const { t } = useTranslation();
  const { register, setValue, formState: { errors } } =
    useFormContext<GuiConfig>();
  const enableMdns = useWatch({ name: "enableMdns" });

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.network")}</CardTitle>
        <CardDescription>{t("settings.network.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <Label htmlFor="settings-quic-port">
            {t("settings.network.quicPort")}
          </Label>
          <Input
            id="settings-quic-port"
            type="number"
            inputMode="numeric"
            min={0}
            max={65535}
            {...register("quicPort", { valueAsNumber: true })}
          />
          <ErrorText code={errors.quicPort?.message} />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="settings-tcp-port">
            {t("settings.network.tcpPort")}
          </Label>
          <Input
            id="settings-tcp-port"
            type="number"
            inputMode="numeric"
            min={0}
            max={65535}
            {...register("tcpPort", { valueAsNumber: true })}
          />
          <ErrorText code={errors.tcpPort?.message} />
        </div>
        <div className="flex items-center justify-between gap-4">
          <div className="flex flex-col">
            <Label htmlFor="settings-mdns">{t("settings.network.mdns")}</Label>
            <p className="text-muted-foreground text-xs">
              {t("settings.network.mdnsHint")}
            </p>
          </div>
          <Switch
            id="settings-mdns"
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
