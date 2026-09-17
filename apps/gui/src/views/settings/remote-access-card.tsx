import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type { I18nKey } from "@/i18n/types";
import { AddressListEditor } from "@/views/shared/address-list-editor";
import type { SettingsFormValues } from "./config-schema";
import { SettingsBlock, SettingsGroup, SettingsRow } from "./settings-row";

// F14 口径（同 network-card/advertise-card）：失焦触发校验，已有错误随输入复验。
function FpsRow() {
  const { t } = useTranslation();
  const {
    register,
    trigger,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  const errorCode = errors.rdFps?.message;
  const errorId = "settings-rd-fps-error";

  return (
    <SettingsRow
      htmlFor="settings-rd-fps"
      label={t("settings.remoteAccess.fps")}
      description={t("settings.remoteAccess.fpsHint")}
      error={
        errorCode != null ? (
          <p
            id={errorId}
            role="alert"
            className="text-destructive text-xs"
            data-testid={errorId}
          >
            {t(`common.validation.${errorCode}` as I18nKey)}
          </p>
        ) : null
      }
      control={
        <Input
          id="settings-rd-fps"
          type="number"
          inputMode="numeric"
          min={1}
          max={60}
          className="w-24"
          aria-invalid={errorCode != null ? true : undefined}
          aria-describedby={errorCode != null ? errorId : undefined}
          {...register("rdFps", {
            valueAsNumber: true,
            onBlur: () => void trigger("rdFps"),
            onChange: () => {
              if (errors.rdFps != null) void trigger("rdFps");
            },
          })}
        />
      }
    />
  );
}

// 远程访问组：GuiConfig 契约三字段（rdRequireApproval/rdFps/tunnelServeAllow）
// 的默认策略编辑；业务页挂载时读此默认值，运行态仍可临时覆盖。
export function RemoteAccessCard() {
  const { t } = useTranslation();
  const { control, setValue } = useFormContext<SettingsFormValues>();
  const approval = useWatch({ name: "rdRequireApproval" });

  return (
    <SettingsGroup
      title={t("settings.cards.remoteAccess")}
      description={t("settings.remoteAccess.hint")}
    >
      <SettingsRow
        htmlFor="settings-rd-approval"
        label={t("settings.remoteAccess.approval")}
        description={t("settings.remoteAccess.approvalHint")}
        control={
          <Switch
            id="settings-rd-approval"
            checked={approval}
            onCheckedChange={(next) =>
              setValue("rdRequireApproval", next, { shouldDirty: true })
            }
          />
        }
      />
      <FpsRow />
      <SettingsBlock>
        <AddressListEditor
          control={control}
          name="tunnelServeAllow"
          label={t("settings.remoteAccess.tunnelAllow")}
          hint={t("settings.remoteAccess.tunnelAllowGuide")}
          placeholder="127.0.0.1:7820"
        />
      </SettingsBlock>
    </SettingsGroup>
  );
}
