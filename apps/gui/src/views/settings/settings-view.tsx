import { useCallback, useEffect, useState } from "react";
import { FormProvider, useForm } from "react-hook-form";

import { PageHeader } from "@/components/page/page-header";
import { Skeleton } from "@/components/ui/skeleton";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { useUnsavedGuard } from "@/views/shared/use-unsaved-guard";
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
// 表单脏状态注册路由守卫；校验失败经 invalidCount 在保存条给出可见反馈。
export function SettingsView() {
  const running = useNodeStore((s) => s.status?.running ?? false);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [invalidCount, setInvalidCount] = useState(0);
  const form = useForm<SettingsFormValues>({
    resolver: settingsResolver,
    defaultValues: EMPTY_SETTINGS,
  });
  const reportInvalid = useCallback((count: number) => setInvalidCount(count), []);
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
        invalidCount={invalidCount}
        onSubmit={requestSave}
        onSaveAndRestart={saveAndRestart}
        onReportSaveError={reportSaveError}
        onReportRestartError={reportRestartError}
      />
    </FormProvider>
  );
}
