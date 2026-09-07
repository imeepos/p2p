import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import type { FieldErrors } from "react-hook-form";
import { FormProvider, useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Skeleton } from "@/components/ui/skeleton";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { useUnsavedGuard } from "@/views/shared/use-unsaved-guard";
import { focusFirstInvalidField } from "./focus-first-error";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  toFormValues,
  type SettingsFormValues,
} from "./config-schema";
import { AdvertiseCard } from "./advertise-card";
import { AppearanceCard } from "./appearance-card";
import { IdentityCard } from "./identity-card";
import { NetworkCard } from "./network-card";
import { ProfileCard } from "./profile-card";
import { SettingsNav, type SettingsSectionId } from "./settings-nav";
import { AboutUpdateCard } from "@/views/update/about-update-card";
import { DocsEntryCard } from "./docs-entry-card";
import { LlmShareEntryCard } from "./llm-share-entry-card";
import { SettingsSaveBar } from "./save-bar";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useSettingsSave } from "./use-settings-save";

type LoadState = "loading" | "ready" | "failed";

function LoadingSkeleton() {
  return (
    <div className="flex flex-col gap-4">
      {[0, 1, 2, 3].map((index) => (
        <Skeleton key={index} className="h-28 w-full" />
      ))}
    </div>
  );
}

// 分节容器常驻挂载（hidden 显隐）：表单值与资料草稿在切签后不丢，
// 路由守卫口径与全卡常挂时代一致。
const SECTIONS: SettingsSectionId[] = ["account", "general", "network", "about"];

function SettingsSections({ active }: { active: SettingsSectionId }) {
  const content: Record<SettingsSectionId, ReactNode> = {
    account: (
      <>
        <ProfileCard />
        <IdentityCard />
      </>
    ),
    general: <AppearanceCard />,
    network: (
      <>
        <NetworkCard />
        <AdvertiseCard />
      </>
    ),
    about: (
      <>
        <AboutUpdateCard />
        <DocsEntryCard />
        <LlmShareEntryCard />
      </>
    ),
  };
  return (
    <>
      {SECTIONS.map((id) => (
        <div
          key={id}
          data-testid={`settings-section-${id}`}
          className={id === active ? "flex flex-col gap-8" : "hidden"}
        >
          {content[id]}
        </div>
      ))}
    </>
  );
}

// 设置页：微信设置式双栏（左分节导航 + 右内容面板 + 底部保存条）。
// 配置表单字段全部位于网络分节，校验失败即切到该分节再聚焦首个错误字段。
export function SettingsView() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running ?? false);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [invalidCount, setInvalidCount] = useState(0);
  const [section, setSection] = useState<SettingsSectionId>("account");
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
  });
  // 校验失败：保存条汇总 + 切到网络分节，渲染完成后二次聚焦（首次聚焦
  // 发生在 hidden 容器内是 no-op），保证「点了保存有反应」可见可定位。
  const reportInvalid = useCallback(
    (count: number, errors: FieldErrors<SettingsFormValues>) => {
      setInvalidCount(count);
      if (count > 0) {
        setSection("network");
        requestAnimationFrame(() => focusFirstInvalidField(errors));
      }
    },
    [],
  );
  const { submitSave, saveAndRestart, reportSaveError, reportRestartError } =
    useSettingsSave(form, reportInvalid);

  // 每次提交前清掉上一轮的校验错误提示，失败时由 reportInvalid 重新给出
  const requestSave = useCallback(async () => {
    setInvalidCount(0);
    await submitSave();
  }, [submitSave]);

  useUnsavedGuard("settings-form", {
    hasUnsaved: () => form.formState.isDirty,
    discard: () => form.reset(),
  });

  const loadConfig = useCallback(async () => {
    form.reset(toFormValues(await ipc.configGet()));
  }, [form]);

  useEffect(() => {
    let cancelled = false;
    loadConfig()
      .then(() => {
        if (!cancelled) setLoadState("ready");
      })
      .catch((error) => {
        console.error("[settings] config_get 失败", error);
        if (!cancelled) setLoadState("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [loadConfig]);

  const retryLoad = useCallback(async () => {
    await loadConfig();
    setLoadState("ready");
  }, [loadConfig]);

  return (
    <FormProvider {...form}>
      <div className="flex min-h-0 flex-1 gap-4" data-testid="settings-layout">
        <SettingsNav active={section} onSelect={setSection} />
        <section
          aria-label={t("settings.title")}
          data-testid="settings-panel"
          className="bg-card ring-border flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg ring-1"
        >
          <div className="min-h-0 flex-1 overflow-y-auto p-6">
            {loadState === "loading" ? <LoadingSkeleton /> : null}
            {loadState === "failed" ? (
              <LoadFailedNotice
                onRetry={retryLoad}
                messageKey="settings.loadFailed"
              />
            ) : null}
            {loadState === "ready" ? (
              <SettingsSections active={section} />
            ) : null}
          </div>
          <div className="border-border border-t px-6">
            <SettingsSaveBar
              dirty={form.formState.isDirty}
              loaded={loadState === "ready"}
              running={running}
              invalidCount={invalidCount}
              onSubmit={requestSave}
              onSaveAndRestart={saveAndRestart}
              onReportSaveError={reportSaveError}
              onReportRestartError={reportRestartError}
            />
          </div>
        </section>
      </div>
    </FormProvider>
  );
}
