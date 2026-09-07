import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";

import { focusFirstInvalidField } from "./focus-first-error";
import { PeerIdField } from "./peer-id-field";
import {
  EMPTY_SHARE_FORM,
  sparePrefillOf,
  validateShareForm,
  type ProviderConfig,
  type ShareErrors,
  type ShareFormValues,
} from "./provider-configs";
import type { LlmShareBackend } from "./types";

function ShareFieldError({ messageKey, htmlId }: { messageKey?: I18nKey; htmlId: string }) {
  const { t } = useTranslation();
  if (!messageKey) return null;
  return (
    <p role="alert" id={htmlId} className="text-destructive text-xs">
      {t(messageKey)}
    </p>
  );
}

// R2-12：字段 id → 错误键映射（聚焦顺序 = 表单视觉顺序）
function shareFieldErrorOf(errors: ShareErrors): Partial<Record<string, I18nKey>> {
  return {
    "llm-provider-share-peer": errors.peerId,
    "llm-provider-share-spare": errors.spare,
    "llm-provider-share-period": errors.periodEnds,
  };
}

interface ProviderShareFormProps {
  config: ProviderConfig;
  backend: LlmShareBackend;
  onShared: () => void;
  onCancel: () => void;
}

// 把一条 provider 配置分享给好友：按配置模型发布能力声明（offerPublish）
// 并把好友 PeerId 按同批模型加入白名单（allow，默认拒绝语义的显式放行）。
// 密钥不参与任何请求——好友经本机代理调用，apiKey 永不出本机。
export function ProviderShareForm({ config, backend, onShared, onCancel }: ProviderShareFormProps) {
  const { t } = useTranslation();
  const [values, setValues] = useState<ShareFormValues>({
    ...EMPTY_SHARE_FORM,
    spareText: sparePrefillOf(config.models),
  });
  const [errors, setErrors] = useState<ShareErrors>({});
  const [peerTouched, setPeerTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const set = (field: keyof ShareFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const validation = validateShareForm(values, config.models);
    setErrors(validation.errors);
    const fieldErrors = shareFieldErrorOf(validation.errors);
    focusFirstInvalidField(Object.keys(fieldErrors), (id) => fieldErrors[id] != null);
    if (!validation.publishReq || !validation.peerId) return;
    setBusy(true);
    setActionError(null);
    try {
      await backend.offerPublish(validation.publishReq);
      await backend.allow({
        peerId: validation.peerId,
        models: config.models,
        note: values.note.trim() || undefined,
      });
      toastSuccess(t("llmShare.providers.shareSuccess"));
      onShared();
    } catch (error) {
      console.error("[llm-share] provider 配置分享失败", error);
      const text = errorText(error);
      setActionError(text);
      toastError(t("llmShare.providers.shareFailed"), {
        description: text,
        context: "llm.provider_share",
      });
    } finally {
      setBusy(false);
    }
  };

  const peerError = peerTouched ? errors.peerId : undefined;

  return (
    <form
      className="flex flex-col gap-3 rounded-md border p-3"
      onSubmit={(e) => void handleSubmit(e)}
      noValidate
      data-testid="provider-share-form"
    >
      <p className="text-muted-foreground text-xs">{t("llmShare.providers.shareHint")}</p>
      <PeerIdField
        label={t("llmShare.providers.sharePeerId")}
        inputId="llm-provider-share-peer"
        value={values.peerId}
        onValueChange={set("peerId")}
        onBlur={() => setPeerTouched(true)}
        errorKey={peerError ?? null}
        errorId="llm-provider-share-peer-error"
      />
      <div className="flex flex-col gap-1">
        <Label htmlFor="llm-provider-share-spare">{t("llmShare.providers.shareSpare")}</Label>
        <Textarea
          id="llm-provider-share-spare"
          rows={config.models.length}
          value={values.spareText}
          onChange={(e) => set("spareText")(e.target.value)}
          placeholder={t("llmShare.providers.shareSparePlaceholder")}
          aria-invalid={errors.spare ? true : undefined}
          aria-describedby={errors.spare ? "llm-provider-share-spare-error" : undefined}
        />
        <ShareFieldError messageKey={errors.spare} htmlId="llm-provider-share-spare-error" />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="llm-provider-share-period">{t("llmShare.providers.sharePeriodEnds")}</Label>
          <Input
            id="llm-provider-share-period"
            type="date"
            value={values.periodEnds}
            onChange={(e) => set("periodEnds")(e.target.value)}
            aria-invalid={errors.periodEnds ? true : undefined}
            aria-describedby={errors.periodEnds ? "llm-provider-share-period-error" : undefined}
          />
          <ShareFieldError messageKey={errors.periodEnds} htmlId="llm-provider-share-period-error" />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="llm-provider-share-note">{t("llmShare.providers.shareNote")}</Label>
          <Input
            id="llm-provider-share-note"
            value={values.note}
            onChange={(e) => set("note")(e.target.value)}
            placeholder={t("llmShare.providers.shareNotePlaceholder")}
          />
        </div>
      </div>
      {actionError ? (
        <p role="alert" className="text-destructive text-xs">
          {t("llmShare.providers.shareFailed")}: {actionError}
        </p>
      ) : null}
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={busy}>
          {t("llmShare.providers.shareSubmit")}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={onCancel}>
          {t("llmShare.providers.cancel")}
        </Button>
      </div>
    </form>
  );
}
