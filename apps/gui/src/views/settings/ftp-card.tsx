import { useFormContext, useWatch } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { ErrorText } from "@/views/shared/error-text";
import type { SettingsFormValues } from "./config-schema";
import { FtpAccountsEditor } from "./ftp-accounts-editor";
import { SettingsBlock, SettingsGroup, SettingsRow } from "./settings-row";

// FTP 服务卡（W2b 契约）：根目录/鉴权开关随主表单草稿编辑，账号表由
// ftp_config_save 整表落盘；效果语义 = 节点重启生效（组头提示注明）。
export function FtpCard() {
  const { t } = useTranslation();
  const {
    register,
    trigger,
    setValue,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  const authz = useWatch({ name: "ftpAuthz" });
  const rootError = errors.ftpRoot?.message;

  return (
    <SettingsGroup
      title={t("settings.ftp.title")}
      description={t("settings.ftp.hint")}
    >
      <SettingsRow
        htmlFor="settings-ftp-root"
        label={t("settings.ftp.root")}
        description={t("settings.ftp.rootHint")}
        error={
          rootError != null ? <ErrorText code={rootError} /> : null
        }
        control={
          <Input
            id="settings-ftp-root"
            className="w-64 font-mono text-xs"
            placeholder="/srv/ftp"
            aria-invalid={rootError != null ? true : undefined}
            {...register("ftpRoot", {
              onBlur: () => void trigger("ftpRoot"),
            })}
          />
        }
      />
      <SettingsRow
        htmlFor="settings-ftp-authz"
        label={t("settings.ftp.authz")}
        description={t("settings.ftp.authzHint")}
        control={
          <Switch
            id="settings-ftp-authz"
            checked={authz}
            onCheckedChange={(next) =>
              setValue("ftpAuthz", next, { shouldDirty: true })
            }
          />
        }
      />
      <SettingsBlock>
        <FtpAccountsEditor />
      </SettingsBlock>
    </SettingsGroup>
  );
}
