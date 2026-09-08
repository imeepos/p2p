import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import type { I18nKey } from "@/i18n/types";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";

import { isOfferNotPublished } from "./offer-errors";
import type { ProviderConfig } from "./provider-configs";
import type { LlmShareBackend, LlmShareCreateResult } from "./types";

const TTL_OPTIONS = [
  { value: "1h", label: "llmShare.share.ttl1h", secs: 3_600 },
  { value: "24h", label: "llmShare.share.ttl24h", secs: 86_400 },
  { value: "7d", label: "llmShare.share.ttl7d", secs: 604_800 },
] as const;

type TtlValue = (typeof TTL_OPTIONS)[number]["value"];
// 提交时刻取现在：模块作用域（渲染树之外），react-hooks/purity 只分析组件/hook 作用域
function nowUnixSecs(): number {
  return Math.floor(Date.now() / 1000);
}


// 生成 dsh-llm-share:// 分享链接（契约 §16.6）：模型须 ⊆ 当前 offer（offerShow 拉声明），
// 有效期 1h/24h/7d 档位缺省 24h；成功展示链接 + 「复制」「发送到聊天」（跳 /chat 预填
// 输入框，链接作为普通文本，不改线协议）。
export function ShareCreateForm({
  provider,
  backend,
  onCreated,
  onCancel,
}: {
  provider: ProviderConfig;
  backend: LlmShareBackend;
  onCreated: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [selected, setSelected] = useState<string[]>(() => [...provider.models]);
  const [ttl, setTtl] = useState<TtlValue>("24h");
  const [note, setNote] = useState("");
  const [offerModels, setOfferModels] = useState<string[] | null>(null);
  const [offerMissing, setOfferMissing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [result, setResult] = useState<LlmShareCreateResult | null>(null);

  // 挂载拉当前 offer：校验分享模型 ⊆ offer.models；未发布=显式提示（常态非故障）
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const offer = await backend.offerShow();
        if (!cancelled) setOfferModels(offer.models);
      } catch (error) {
        if (cancelled) return;
        if (isOfferNotPublished(error)) {
          setOfferMissing(true);
          return;
        }
        console.warn("[llm-share] offer 拉取失败，模型 ⊆ 校验受限", error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const toggleModel = (model: string) =>
    setSelected((prev) =>
      prev.includes(model) ? prev.filter((m) => m !== model) : [...prev, model],
    );

  const outOfOffer = offerModels ? selected.find((m) => !offerModels.includes(m)) : null;
  const validationErrorKey: I18nKey | null =
    selected.length === 0
      ? "llmShare.share.modelNone"
      : outOfOffer
        ? "llmShare.share.modelNotInOffer"
        : null;

  const handleSubmit = async (event: FormEvent, nowSecs: number) => {
    event.preventDefault();
    if (selected.length === 0) {
      setActionError(t("llmShare.share.modelNone"));
      return;
    }
    if (outOfOffer) {
      setActionError(t("llmShare.share.modelNotInOffer", { model: outOfOffer }));
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      const ttlSecs = TTL_OPTIONS.find((o) => o.value === ttl)?.secs ?? 86_400;
      const created = await backend.shareCreate({
        providerId: provider.id,
        models: selected,
        expiresAt: nowSecs + ttlSecs,
        note: note.trim() || undefined,
      });
      setResult(created);
    } catch (error) {
      console.error("[llm-share] 生成分享链接失败", error);
      const text = errorText(error);
      setActionError(text);
      toastError(t("llmShare.share.createFailed"), { description: text });
    } finally {
      setBusy(false);
    }
  };

  const copyLink = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.link);
      toastSuccess(t("llmShare.share.copied"));
    } catch (error) {
      console.warn("[llm-share] 复制分享链接失败", error);
      toastError(t("llmShare.share.createFailed"), { description: errorText(error) });
    }
  };

  const sendToChat = () => {
    if (!result) return;
    // 跳 /chat 并把链接文本填入输入框（?compose= 深链预填，普通文本消息）
    navigate(`/chat?compose=${encodeURIComponent(result.link)}`);
    toastSuccess(t("llmShare.share.sentToChat"));
  };

  return (
    <div className="flex flex-col gap-3 rounded-md border p-3" data-testid="share-create">
      <p className="text-sm font-medium">{t("llmShare.share.createTitle")}</p>
      {result ? (
        <div className="flex flex-col gap-3" data-testid="share-create-result">
          <p className="text-muted-foreground text-xs">{t("llmShare.share.createHint")}</p>
          <div className="flex flex-col gap-1">
            <Label htmlFor="share-link-value">{t("llmShare.share.linkLabel")}</Label>
            <Input
              id="share-link-value"
              readOnly
              value={result.link}
              data-testid="share-link-value"
              className="font-mono text-xs"
            />
          </div>
          <div className="flex gap-2">
            <Button type="button" size="sm" onClick={() => void copyLink()} data-testid="share-link-copy">
              {t("llmShare.share.copy")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={sendToChat} data-testid="share-link-send-chat">
              {t("llmShare.share.sendToChat")}
            </Button>
            <Button type="button" size="sm" variant="ghost" onClick={onCreated}>
              {t("llmShare.share.close")}
            </Button>
          </div>
        </div>
      ) : (
        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => void handleSubmit(e, nowUnixSecs())}
          noValidate
          data-testid="share-create-form"
        >
          {offerMissing ? (
            <p role="alert" className="text-destructive text-xs" data-testid="share-offer-missing">
              {t("llmShare.share.offerMissing")}
            </p>
          ) : null}
          <div className="flex flex-col gap-1">
            <span className="text-sm font-medium">{t("llmShare.share.modelLabel")}</span>
            <div className="flex flex-wrap gap-3" data-testid="share-model-options">
              {provider.models.map((model) => (
                <label
                  key={model}
                  className="flex cursor-pointer items-center gap-1.5 text-sm"
                  data-testid={"share-model-" + model}
                >
                  <input
                    type="checkbox"
                    checked={selected.includes(model)}
                    onChange={() => toggleModel(model)}
                    disabled={offerMissing}
                  />
                  {model}
                </label>
              ))}
            </div>
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-sm font-medium">{t("llmShare.share.ttlLabel")}</span>
            <SegmentedControl
              value={ttl}
              onChange={setTtl}
              options={TTL_OPTIONS.map((o) => ({ value: o.value, label: t(o.label) }))}
              ariaLabel={t("llmShare.share.ttlLabel")}
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="share-note">{t("llmShare.share.noteLabel")}</Label>
            <Textarea
              id="share-note"
              rows={2}
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={t("llmShare.share.notePlaceholder")}
            />
          </div>
          {validationErrorKey ? (
            <p role="alert" className="text-destructive text-xs" data-testid="share-validation-error">
              {outOfOffer
                ? t("llmShare.share.modelNotInOffer", { model: outOfOffer })
                : t(validationErrorKey)}
            </p>
          ) : null}
          {actionError ? (
            <p role="alert" className="text-destructive text-xs" data-testid="share-action-error">
              {actionError}
            </p>
          ) : null}
          <div className="flex gap-2">
            <Button type="submit" size="sm" disabled={busy || offerMissing}>
              {busy ? t("llmShare.share.generating") : t("llmShare.share.submit")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={onCancel}>
              {t("llmShare.providers.cancel")}
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
