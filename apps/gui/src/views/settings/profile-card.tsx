import { ChangeEvent, useCallback, useEffect, useRef, useState } from "react";
import { CircleUserRoundIcon, ImageUpIcon, Trash2Icon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

const AVATAR_INPUT_ACCEPT = "image/png,image/jpeg,image/webp";

function AvatarPreview({ src, alt }: { src: string | null; alt: string }) {
  if (src) {
    return (
      <img
        src={src}
        alt={alt}
        className="bg-muted size-16 shrink-0 rounded-full object-cover"
      />
    );
  }
  return (
    <span className="bg-muted text-muted-foreground flex size-16 shrink-0 items-center justify-center rounded-full">
      <CircleUserRoundIcon aria-hidden className="size-8" />
    </span>
  );
}

// 节点资料卡：name/description/avatar 可视化编辑；与网络配置表单解耦，
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
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.profile")}</CardTitle>
        <CardDescription>{t("settings.profile.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {loadError !== null && !loaded ? (
          <LoadFailedNotice onRetry={retryLoad} messageKey="settings.profile.loadFailed" />
        ) : (
          <>
            <div className="flex items-center gap-4">
              <AvatarPreview src={current.avatar} alt={t("settings.profile.avatarAlt")} />
              <div className="flex flex-col gap-1.5">
                <Label>{t("settings.profile.avatar")}</Label>
                <div className="flex gap-2">
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
                <p className="text-foreground/70 text-xs">
                  {t("settings.profile.avatarHint")}
                </p>
              </div>
              <input
                ref={fileRef}
                type="file"
                accept={AVATAR_INPUT_ACCEPT}
                className="hidden"
                onChange={(e) => void onPickFile(e)}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="profile-name">{t("settings.profile.name")}</Label>
              <Input
                id="profile-name"
                value={current.name}
                maxLength={NAME_MAX_CHARS}
                placeholder={t("settings.profile.namePlaceholder")}
                onChange={(e) => setDraft({ ...current, name: e.target.value })}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="profile-description">
                {t("settings.profile.description")}
              </Label>
              <Textarea
                id="profile-description"
                value={current.description}
                maxLength={DESCRIPTION_MAX_CHARS}
                placeholder={t("settings.profile.descriptionPlaceholder")}
                onChange={(e) => setDraft({ ...current, description: e.target.value })}
              />
            </div>
            <div className="flex justify-end">
              <Button
                type="button"
                size="sm"
                disabled={!dirty || saving}
                onClick={() => void onSave()}
              >
                {saving ? t("settings.profile.saving") : t("settings.profile.save")}
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
