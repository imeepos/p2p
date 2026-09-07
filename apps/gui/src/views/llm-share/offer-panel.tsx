import { CircleSlash, RefreshCwIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";

import { isOfferNotPublished, warnOfferLoadOnce } from "./offer-errors";
import { OfferPublishForm } from "./offer-publish-form";
import { OfferStatusCard } from "./offer-status-card";
import type { LlmOfferView, LlmShareBackend } from "./types";

// 能力发布面板（契约 §16.3）：信息优先——首屏是声明状态卡（或空态 CTA），
// 发布/更新表单点按钮才出现；更新场景表单按当前声明回填，改个数即可重发。
export function OfferPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [offer, setOffer] = useState<LlmOfferView | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [formOpen, setFormOpen] = useState(false);

  // R2-03：未发布是常态非故障——空态静默走中文出路文案；仅真实错误才
  // 原样露出并告警（会话级单次，console 不逐次刷屏）。
  const reportLoadError = useCallback((error: unknown) => {
    if (isOfferNotPublished(error)) {
      setLoadError(null);
      return;
    }
    warnOfferLoadOnce(error);
    setLoadError(errorText(error));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const view = await backend.offerShow();
      setOffer(view);
      setLoadError(null);
    } catch (error) {
      reportLoadError(error);
      setOffer(null);
    } finally {
      setLoaded(true);
    }
  }, [backend, reportLoadError]);

  // 挂载拉取走 effect 内联 IIFE（react-hooks/set-state-in-effect 合规形态，
  // use-gui-config 先例）；refresh 供按钮手动刷新复用。
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const view = await backend.offerShow();
        if (!cancelled) {
          setOffer(view);
          setLoadError(null);
        }
      } catch (error) {
        if (!cancelled) reportLoadError(error);
      } finally {
        if (!cancelled) setLoaded(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend, reportLoadError]);

  const handlePublished = (view: LlmOfferView) => {
    setOffer(view);
    setLoadError(null);
    setFormOpen(false);
  };

  return (
    <div className="flex flex-col gap-3" data-testid="offer-panel">
      {offer ? (
        <>
          <OfferStatusCard offer={offer} />
          {!formOpen ? (
            <div className="flex gap-2">
              <Button type="button" size="sm" onClick={() => setFormOpen(true)} data-testid="offer-update">
                {t("llmShare.offer.updateCta")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void refresh()}
                data-testid="offer-refresh"
              >
                <RefreshCwIcon aria-hidden className="size-3.5" />
                {t("llmShare.offer.refresh")}
              </Button>
            </div>
          ) : null}
        </>
      ) : loaded ? (
        <EmptyState
          icon={CircleSlash}
          title={t("llmShare.offer.emptyTitle")}
          description={loadError ?? t("llmShare.offer.emptyHint")}
          action={
            !formOpen ? (
              <Button type="button" size="sm" onClick={() => setFormOpen(true)} data-testid="offer-publish-cta">
                {t("llmShare.offer.publishCta")}
              </Button>
            ) : undefined
          }
        />
      ) : null}
      {formOpen ? (
        <OfferPublishForm
          offer={offer}
          backend={backend}
          onPublished={handlePublished}
          onCancel={() => setFormOpen(false)}
        />
      ) : null}
    </div>
  );
}
