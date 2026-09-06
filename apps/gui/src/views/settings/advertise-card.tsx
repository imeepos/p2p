import { useFormContext } from "react-hook-form";
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
import type { I18nKey } from "@/i18n/types";
import type { SettingsFormValues } from "./config-schema";
import { FactoryDefaultsNotice } from "@/views/shared/factory-defaults-notice";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";

// 宣告与观测卡：advertisedAddrs 列表 + 可空观测端口 + observationAddrs 列表。
// advertisedAddrs 无出厂默认，不提供恢复入口。bootstrap/relay 的编辑入口在
// 发现页（rendezvous 地址簿）与中继页（中继地址配置），设置页不再重复。
export function AdvertiseCard() {
  const { t } = useTranslation();
  const {
    control,
    register,
    trigger,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  // F14 口径（同 network-card PortField）：失焦触发校验，已有错误随输入复验。
  const obsErrorCode = errors.observationPort?.message;
  const obsErrorId = "settings-observation-port-error";

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.advertise")}</CardTitle>
        <CardDescription>{t("settings.advertise.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <AddressListEditor
          control={control}
          name="advertisedAddrs"
          label={t("settings.advertise.advertisedAddrs")}
          hint={t("settings.advertise.advertisedAddrsGuide")}
          placeholder="203.0.113.5/u3400"
        />
        <div className="flex flex-col gap-1">
          <Label htmlFor="settings-observation-port">
            {t("settings.advertise.observationPort")}
          </Label>
          <Input
            id="settings-observation-port"
            type="number"
            inputMode="numeric"
            min={1}
            max={65535}
            placeholder={t("settings.advertise.observationPortPlaceholder")}
            aria-invalid={obsErrorCode != null ? true : undefined}
            aria-describedby={obsErrorCode != null ? obsErrorId : undefined}
            {...register("observationPort", {
              valueAsNumber: true,
              onBlur: () => void trigger("observationPort"),
              onChange: () => {
                if (errors.observationPort != null)
                  void trigger("observationPort");
              },
            })}
          />
          {obsErrorCode != null ? (
            <p
              id={obsErrorId}
              role="alert"
              className="text-destructive text-xs"
              data-testid={obsErrorId}
            >
              {t(`common.validation.${obsErrorCode}` as I18nKey)}
            </p>
          ) : null}
          <p className="text-muted-foreground text-xs">
            {t("settings.advertise.observationPortHint")}
          </p>
          <p className="text-muted-foreground text-xs leading-5">
            {t("settings.advertise.observationPortGuide")}
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <AddressListEditor
            control={control}
            name="observationAddrs"
            label={t("settings.advertise.observationAddrs")}
            hint={t("settings.advertise.observationAddrsGuide")}
            placeholder="203.0.113.5:3402"
          />
          <FactoryDefaultsNotice name="observationAddrs" />
        </div>
      </CardContent>
    </Card>
  );
}
