import { TriangleAlertIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import {
  ACTION_CANCELLED_MARK,
  FORM_VALIDATION_MARK,
  isFlowMark,
} from "@/views/shared/form-flow";

interface SettingsSaveBarProps {
  dirty: boolean;
  loaded: boolean;
  running: boolean;
  onSubmit: () => Promise<void>;
  onSaveAndRestart: () => Promise<void>;
  onReportSaveError: (error: unknown) => void;
  onReportRestartError: (error: unknown) => void;
}

// 底部保存条：脏状态提示 + 保存按钮；节点运行中显示重启生效提示与组合按钮。
// 校验失败/用户取消以流标记中断，跳过错误上报（校验错误已内联展示）。
export function SettingsSaveBar({
  dirty,
  loaded,
  running,
  onSubmit,
  onSaveAndRestart,
  onReportSaveError,
  onReportRestartError,
}: SettingsSaveBarProps) {
  const { t } = useTranslation();

  return (
    <>
      {running ? (
        <div className="border-warning/50 bg-warning/10 col-span-12 flex items-start gap-3 rounded-md border p-3 text-sm">
          <TriangleAlertIcon
            className="mt-0.5 size-4 shrink-0 text-warning"
            aria-hidden
          />
          <div className="flex flex-1 flex-col gap-2 sm:flex-row sm:items-center">
            <p className="flex-1">{t("settings.saveBar.runningNotice")}</p>
            <AsyncButton
              type="button"
              size="sm"
              variant="outline"
              action={onSaveAndRestart}
              loadingLabel={t("settings.saveBar.saveAndRestarting")}
              onError={(error) => {
                if (isFlowMark(error, ACTION_CANCELLED_MARK)) return;
                if (isFlowMark(error, FORM_VALIDATION_MARK)) return;
                onReportRestartError(error);
              }}
            >
              {t("settings.saveBar.saveAndRestart")}
            </AsyncButton>
          </div>
        </div>
      ) : null}
      <div className="bg-background/95 col-span-12 sticky bottom-0 z-10 flex items-center justify-between gap-4 border-t py-3 backdrop-blur">
        {/* IM-V2 S6：提示与保存同行（justify-between），对比度提至 AA */}
        <p className="text-xs text-gray-600 dark:text-gray-300">
          {dirty ? t("settings.saveBar.dirty") : t("settings.saveBar.clean")}
        </p>
        <AsyncButton
          type="button"
          disabled={!dirty || !loaded}
          action={onSubmit}
          loadingLabel={t("settings.saveBar.saving")}
          onError={(error) => {
            if (isFlowMark(error, FORM_VALIDATION_MARK)) return;
            onReportSaveError(error);
          }}
        >
          {t("settings.saveBar.save")}
        </AsyncButton>
      </div>
    </>
  );
}
