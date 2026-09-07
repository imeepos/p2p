import { ChangeEvent, useCallback, useEffect, useRef, useState } from "react";
import { CircleUserRoundIcon, ImageUpIcon, Trash2Icon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { AvatarFileError, fileToAvatarDataUrl } from "@/lib/avatar";
import type { NodeProfile } from "@/lib/ipc-types";
import {
  DESCRIPTION_MAX_CHARS,
  NAME_MAX_CHARS,
  validateNodeProfile,
} from "@/lib/profile-rules";
import { useProfileStore } from "@/stores/profile-store";
import { errorText } from "@/views/shared/form-flow";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useUnsavedGuard } from "@/views/shared/use-unsaved-guard";
import { SettingsGroup, SettingsRow } from "./settings-row";

const AVATAR_INPUT_ACCEPT = "image/png,image/jpeg,image/webp";

function AvatarPreview({ src, alt }: { src: string | null; alt: string }) {
  if (src) {
    return (
      <img
        src={src}
        alt={alt}
        className="bg-muted size-14 shrink-0 rounded-full object-cover"
      />
    );
  }
  return (
    <span className="bg-muted text-muted-foreground flex size-14 shrink-0 items-center justify-center rounded-full">
      <CircleUserRoundIcon aria-hidden className="size-7" />
    </span>
  );
}

// 节点资料组：name/description/avatar 可视化编辑；与网络配置表单解耦，
// 资料保存即时生效、无需重启节点（契约 v6 §11）。
export function ProfileCard() {
  const { t } = useTranslation();
  const profile = useProfileStore((s) => s.profile);
  const loaded = useProfileStore((s) => s.loaded);
  const loadError = useProfileStore((s) => s.loadError);
  const load = useProfileStore((s) => s.load);
  const save = useProfileStore((s) => s.save);
  const [draft, setDraft] = useState<NodeProfile | null>(null);
  const [saving, setSaving] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!loaded && loadError === null) {
      // 失败已由 store 留信号（console + loadError），此处仅阻断 unhandled。
      load().catch(() => {});
    }
  }, [loaded, loadError, load]);

  const current = draft ?? profile;
  const dirty =
    draft !== null &&
    (draft.name !== profile.name ||
      draft.description !== profile.description ||
      draft.avatar !== profile.avatar);

  // 资料草稿脏状态注册路由守卫：离开设置页时统一弹确认，放弃则丢弃草稿
  useUnsavedGuard("settings-profile", {
    hasUnsaved: () => dirty,
    discard: () => setDraft(null),
  });

  const retryLoad = useCallback(() => load(), [load]);

  const onPickFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      const avatar = await fileToAvatarDataUrl(file);
      setDraft({ ...current, avatar });
    } catch (error) {
      const tooLarge = error instanceof AvatarFileError && error.code === "avatarTooLarge";
      const messageKey = tooLarge
        ? "settings.profile.avatarTooLarge"
        : "settings.profile.avatarInvalid";
      console.error("[settings] 头像处理失败", error);
      toastError(t(messageKey), { context: "settings.avatar" });
    }
  };

  const onSave = async () => {
    const invalid = validateNodeProfile(current);
    if (invalid) {
      toastError(t("settings.profile.saveFailed"), {
        description: invalid,
        context: "settings.profile_save",
      });
      return;
    }
    setSaving(true);
    try {
      // 表单侧负责 trim（契约 §11：后端校验 trim 后长度，原样落盘）。
      await save({
        name: current.name.trim(),
        description: current.description.trim(),
        avatar: current.avatar,
      });
      setDraft(null);
      toastSuccess(t("settings.profile.saved"));
    } catch (error) {
      console.error("[settings] profile_save 失败", error);
      toastError(t("settings.profile.saveFailed"), {
        description: errorText(error),
        context: "settings.profile_save",
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <SettingsGroup
      title={t("settings.cards.profile")}
      description={t("settings.profile.hint")}
    >
      {loadError !== null && !loaded ? (
        <div className="py-3">
          <LoadFailedNotice
            onRetry={retryLoad}
            messageKey="settings.profile.loadFailed"
          />
        </div>
      ) : (
        <>
          <div className="flex items-center justify-between gap-6 py-3">
            <div className="flex min-w-0 items-center gap-4">
              <AvatarPreview
                src={current.avatar}
                alt={t("settings.profile.avatarAlt")}
              />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="truncate text-sm font-medium">
                  {current.name || t("settings.profile.unnamed")}
                </span>
                <span className="text-muted-foreground text-xs leading-5">
                  {t("settings.profile.avatarHint")}
                </span>
              </div>
            </div>
            <div className="flex shrink-0 gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => fileRef.current?.click()}
              >
                <ImageUpIcon aria-hidden />
                {t("settings.profile.avatarUpload")}
              </Button>
              {current.avatar ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => setDraft({ ...current, avatar: null })}
                >
                  <Trash2Icon aria-hidden />
                  {t("settings.profile.avatarRemove")}
                </Button>
              ) : null}
            </div>
          </div>
          <SettingsRow
            htmlFor="profile-name"
            label={t("settings.profile.name")}
            control={
              <Input
                id="profile-name"
                className="w-56"
                value={current.name}
                maxLength={NAME_MAX_CHARS}
                placeholder={t("settings.profile.namePlaceholder")}
                onChange={(e) => setDraft({ ...current, name: e.target.value })}
              />
            }
          />
          <SettingsRow
            htmlFor="profile-description"
            label={t("settings.profile.description")}
            control={
              <Textarea
                id="profile-description"
                className="h-20 w-80"
                value={current.description}
                maxLength={DESCRIPTION_MAX_CHARS}
                placeholder={t("settings.profile.descriptionPlaceholder")}
                onChange={(e) =>
                  setDraft({ ...current, description: e.target.value })
                }
              />
            }
          />
          <div className="flex justify-end py-3">
            <Button
              type="button"
              size="sm"
              disabled={!dirty || saving}
              onClick={() => void onSave()}
            >
              {saving ? t("settings.profile.saving") : t("settings.profile.save")}
            </Button>
          </div>
          <input
            ref={fileRef}
            type="file"
            accept={AVATAR_INPUT_ACCEPT}
            className="hidden"
            onChange={(e) => void onPickFile(e)}
          />
        </>
      )}
    </SettingsGroup>
  );
}
