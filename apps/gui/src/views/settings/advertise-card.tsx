import { useFormContext } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import type { I18nKey } from "@/i18n/types";
import type { SettingsFormValues } from "./config-schema";
import { FactoryDefaultsNotice } from "@/views/shared/factory-defaults-notice";
import {
  AddressListEditor,
} from "@/views/shared/address-list-editor";
import { SettingsBlock, SettingsGroup, SettingsRow } from "./settings-row";

// 宣告与观测组：advertisedAddrs 列表 + 可空观测端口 + observationAddrs 列表。
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
    <SettingsGroup
      title={t("settings.cards.advertise")}
      description={t("settings.advertise.hint")}
    >
      <SettingsBlock>
        <AddressListEditor
          control={control}
          name="advertisedAddrs"
          label={t("settings.advertise.advertisedAddrs")}
          hint={t("settings.advertise.advertisedAddrsGuide")}
          placeholder="203.0.113.5/u3400"
        />
      </SettingsBlock>
      <SettingsRow
        htmlFor="settings-observation-port"
        label={t("settings.advertise.observationPort")}
        description={
          <>
            <span className="block">
              {t("settings.advertise.observationPortHint")}
            </span>
            <span className="block">
              {t("settings.advertise.observationPortGuide")}
            </span>
          </>
        }
        error={
          obsErrorCode != null ? (
            <p
              id={obsErrorId}
              role="alert"
              className="text-destructive text-xs"
              data-testid={obsErrorId}
            >
              {t(`common.validation.${obsErrorCode}` as I18nKey)}
            </p>
          ) : null
        }
        control={
          <Input
            id="settings-observation-port"
            type="number"
            inputMode="numeric"
            min={1}
            max={65535}
            className="w-40"
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
        }
      />
      <SettingsBlock>
        <AddressListEditor
          control={control}
          name="observationAddrs"
          label={t("settings.advertise.observationAddrs")}
          hint={t("settings.advertise.observationAddrsGuide")}
          placeholder="203.0.113.5:3402"
        />
        <FactoryDefaultsNotice name="observationAddrs" />
      </SettingsBlock>
    </SettingsGroup>
  );
}
