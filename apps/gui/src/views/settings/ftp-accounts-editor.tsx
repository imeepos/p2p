import { PlusIcon, Trash2Icon } from "lucide-react";
import { useFieldArray, useFormContext } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ErrorText } from "@/views/shared/error-text";
import type { FieldErrors, FieldValues } from "react-hook-form";
import type { SettingsFormValues } from "./config-schema";

interface RowError {
  message?: string;
}

function rowMessages(container: unknown, index: number): {
  user?: string;
  password?: string;
} {
  if (!Array.isArray(container)) return {};
  const row = container[index] as
    | { user?: RowError; password?: RowError }
    | undefined;
  return { user: row?.user?.message, password: row?.password?.message };
}

function rootMessage(container: unknown): string | undefined {
  if (!container || Array.isArray(container)) return undefined;
  return (container as { root?: RowError }).root?.message;
}

// FTP 账号表行编辑器：用户名 + 密码（type=password，仅写入不回显）双输入行。
// 既有用户行密码留空 = 保存发空串 = 后端保留原密码（W2b 契约钉死语义），
// 新增行空密码由 config-schema 校验拦截；行级红字 + root 级重复用户名提示。
export function FtpAccountsEditor() {
  const { t } = useTranslation();
  const {
    control,
    register,
    formState: { errors },
  } = useFormContext<SettingsFormValues>();
  const { fields, append, remove } = useFieldArray({
    control,
    name: "ftpAccounts",
  });
  const container = errors.ftpAccounts as FieldErrors<FieldValues> | undefined;

  return (
    <div className="flex flex-col gap-2" data-field="ftpAccounts">
      <div className="flex flex-col gap-0.5">
        <Label>{t("settings.ftp.accounts")}</Label>
        <p className="text-muted-foreground text-xs">
          {t("settings.ftp.accountsHint")}
        </p>
      </div>
      {fields.length === 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("common.table.empty")}
        </p>
      ) : (
        fields.map((field, index) => {
          const messages = rowMessages(container, index);
          const rowLabel = t("settings.ftp.rowLabel", { index: index + 1 });
          return (
            <div key={field.id} className="flex flex-col gap-1">
              <Label
                htmlFor={`settings-ftp-user-${index}`}
                className="text-muted-foreground text-xs"
              >
                {rowLabel}
              </Label>
              <div className="flex items-center gap-2">
                <Input
                  id={`settings-ftp-user-${index}`}
                  className="w-40 font-mono text-xs"
                  placeholder={t("settings.ftp.userPlaceholder")}
                  aria-label={`${rowLabel} ${t("settings.ftp.userPlaceholder")}`}
                  {...register(`ftpAccounts.${index}.user` as never)}
                />
                <Input
                  type="password"
                  autoComplete="off"
                  className="flex-1 font-mono text-xs"
                  placeholder={t("settings.ftp.passwordPlaceholder")}
                  aria-label={`${rowLabel} ${t("settings.ftp.passwordPlaceholder")}`}
                  {...register(`ftpAccounts.${index}.password` as never)}
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={`${t("settings.ftp.removeAccount")} ${rowLabel}`}
                  onClick={() => remove(index)}
                >
                  <Trash2Icon aria-hidden />
                </Button>
              </div>
              <ErrorText code={messages.user} />
              <ErrorText code={messages.password} />
            </div>
          );
        })
      )}
      <ErrorText code={rootMessage(container)} />
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="w-fit"
        onClick={() => append({ user: "", password: "", existing: false })}
      >
        <PlusIcon aria-hidden />
        {t("settings.ftp.addAccount")}
      </Button>
    </div>
  );
}
