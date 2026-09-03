import { useCallback, useEffect, useState } from "react";
import { FormProvider, useForm } from "react-hook-form";

import { PageHeader } from "@/components/page/page-header";
import { Skeleton } from "@/components/ui/skeleton";
import { ipc } from "@/lib/ipc";
import { AdvertiseCard } from "./advertise-card";
import { AppearanceCard } from "./appearance-card";
import {
  EMPTY_SETTINGS,
  settingsResolver,
  toFormValues,
  type SettingsFormValues,
} from "./config-schema";
import { IdentityCard } from "./identity-card";
import { NetworkCard } from "./network-card";
import { ProfileCard } from "./profile-card";
import { AboutUpdateCard } from "@/views/update/about-update-card";
import {
  SettingsSaveBar,
} from "./save-bar";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useNodeStore } from "@/stores/node-store";
import { useSettingsSave } from "./use-settings-save";

type LoadState = "loading" | "ready" | "failed";

function LoadingSkeleton() {
  return (
    <div className="col-span-12 grid grid-cols-12 gap-4">
      {[0, 1, 2, 3].map((index) => (
        <Skeleton key={index} className="col-span-12 h-48 lg:col-span-6" />
      ))}
    </div>
  );
}

function SettingsCards() {
  return (
    <>
      <NetworkCard />
      <AdvertiseCard />
      <AppearanceCard />
      <ProfileCard />
      <IdentityCard />
      <AboutUpdateCard />
    </>
  );
}

// 设置页：分卡表单 + 底部保存条；加载失败可重试，保存逻辑在 use-settings-save。
export function SettingsView() {
  const running = useNodeStore((s) => s.status?.running ?? false);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
  });
  const { submitSave, saveAndRestart, reportSaveError, reportRestartError } =
    useSettingsSave(form);

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
      <PageHeader
        titleKey="settings.title"
        descriptionKey="settings.description"
      />
      {loadState === "loading" ? <LoadingSkeleton /> : null}
      {loadState === "failed" ? (
        <LoadFailedNotice onRetry={retryLoad} messageKey="settings.loadFailed" />
      ) : null}
      {loadState === "ready" ? <SettingsCards /> : null}
      <SettingsSaveBar
        dirty={form.formState.isDirty}
        loaded={loadState === "ready"}
        running={running}
        onSubmit={submitSave}
        onSaveAndRestart={saveAndRestart}
        onReportSaveError={reportSaveError}
        onReportRestartError={reportRestartError}
      />
    </FormProvider>
  );
}
