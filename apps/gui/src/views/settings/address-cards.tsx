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
import { FactoryDefaultsNotice } from "./factory-defaults-notice";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";
import { ErrorText } from "@/views/shared/error-text";

export function BootstrapCard() {
  const { t } = useTranslation();
  const { control } = useFormContext<SettingsFormValues>();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.bootstrap")}</CardTitle>
        <CardDescription>{t("settings.bootstrap.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <AddressListEditor
          control={control}
          name="bootstrap"
          label={t("settings.bootstrap.label")}
          placeholder="192.168.1.10/u3400"
        />
        <FactoryDefaultsNotice name="bootstrap" />
      </CardContent>
    </Card>
  );
}

export function RelayAddrsCard() {
  const { t } = useTranslation();
  const { control } = useFormContext<SettingsFormValues>();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.relay")}</CardTitle>
        <CardDescription>{t("settings.relayCard.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <AddressListEditor
          control={control}
          name="relayAddrs"
          label={t("settings.relayCard.label")}
          placeholder="192.168.1.10/u3402"
        />
        <FactoryDefaultsNotice name="relayAddrs" />
      </CardContent>
    </Card>
  );
}

// 宣告与观测卡：advertisedAddrs 列表 + 可空观测端口 + observationAddrs 列表。
// advertisedAddrs 无出厂默认，不提供恢复入口。
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
        </div>
        <div className="flex flex-col gap-2">
          <AddressListEditor
            control={control}
            name="observationAddrs"
            label={t("settings.advertise.observationAddrs")}
            placeholder="203.0.113.5:3402"
          />
          <FactoryDefaultsNotice name="observationAddrs" />
        </div>
      </CardContent>
    </Card>
  );
}
