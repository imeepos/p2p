import { useCallback } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { UseFormReturn } from "react-hook-form";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import {
  ACTION_CANCELLED_MARK,
  FORM_VALIDATION_MARK,
  errorText,
} from "@/views/shared/form-flow";
import { toFormValues, toGuiConfig, type SettingsFormValues } from "./config-schema";

function reportSaveFailure(t: TFunction, error: unknown): void {
  console.error("[settings] config_save 失败", error);
  toastError(t("settings.saveBar.saveFailed"), {
    description: errorText(error),
    context: "settings.config_save",
  });
}

function reportRestartFailure(t: TFunction, error: unknown): void {
  console.error("[settings] 保存并重启失败", error);
  toastError(t("settings.saveBar.restartFailed"), {
    description: errorText(error),
    context: "settings.save_restart",
  });
}

// 设置页保存流：保存后回读重置（往返闭环）；组合按钮走确认->保存->stop->start。
export function useSettingsSave(form: UseFormReturn<SettingsFormValues>) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const stopNode = useNodeStore((s) => s.stopNode);
  const startNode = useNodeStore((s) => s.startNode);

  // 保存成功后回读并重置表单：脏状态归零，值与持久层一致。
  const saveAndReload = useCallback(
    async (values: SettingsFormValues) => {
      await ipc.configSave(toGuiConfig(values));
      markLocalWrite("config");
      form.reset(toFormValues(await ipc.configGet()));
      toastSuccess(t("settings.saveBar.saved"));
    },
    [form, t],
  );

  const submitSave = useCallback((): Promise<void> => {
    return new Promise((resolve, reject) => {
      void form.handleSubmit(
        async (values) => {
          await saveAndReload(values);
          resolve();
        },
        () => reject(new Error(FORM_VALIDATION_MARK)),
      )();
    });
  }, [form, saveAndReload]);

  const saveAndRestart = useCallback(async () => {
    const ok = await confirm({
      title: t("settings.saveBar.restartConfirmTitle"),
      description: t("settings.saveBar.restartConfirmDesc"),
      confirmText: t("settings.saveBar.restartConfirmYes"),
      cancelText: t("common.actions.cancel"),
    });
    if (!ok) throw new Error(ACTION_CANCELLED_MARK);
    if (!(await form.trigger())) throw new Error(FORM_VALIDATION_MARK);
    const values = toGuiConfig(form.getValues());
    await ipc.configSave(values);
    markLocalWrite("config");
    await stopNode();
    await startNode(values);
    form.reset(toFormValues(await ipc.configGet()));
    toastSuccess(t("settings.saveBar.restartDone"));
  }, [confirm, form, startNode, stopNode, t]);

  return {
    submitSave,
    saveAndRestart,
    reportSaveError: (error: unknown) => reportSaveFailure(t, error),
    reportRestartError: (error: unknown) => reportRestartFailure(t, error),
  };
}