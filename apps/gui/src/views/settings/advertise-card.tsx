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
import type { SettingsFormValues } from "./config-schema";
import { FactoryDefaultsNotice } from "@/views/shared/factory-defaults-notice";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";
import { ErrorText } from "@/views/shared/error-text";

// 宣告与观测卡：advertisedAddrs 列表 + 可空观测端口 + observationAddrs 列表。
// advertisedAddrs 无出厂默认，不提供恢复入口。bootstrap/relay 的编辑入口在
// 发现页（rendezvous 地址簿）与中继页（中继地址配置），设置页不再重复。
export function AdvertiseCard() {
  const { t } = useTranslation();
  const {
    control,
    register,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();

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
            {...register("observationPort", { valueAsNumber: true })}
          />
          <ErrorText code={errors.observationPort?.message} />
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
