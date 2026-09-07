import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { PageHeader } from "@/components/page/page-header";

import { AllowlistPanel } from "./allowlist-panel";
import { BorrowPanel } from "./borrow-panel";
import { LedgerPanel } from "./ledger-panel";
import { OfferPanel } from "./offer-panel";
import { ProviderPanel } from "./provider-panel";
import type { LlmShareBackend } from "./types";

function PanelSection({
  titleKey,
  testid,
  children,
}: {
  titleKey:
    | "llmShare.panels.offer"
    | "llmShare.panels.providers"
    | "llmShare.panels.allowlist"
    | "llmShare.panels.borrow"
    | "llmShare.panels.ledger";
  testid: string;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <section className="flex min-h-0 flex-col gap-3" data-testid={testid}>
      <h2 className="text-base font-semibold">{t(titleKey)}</h2>
      {children}
    </section>
  );
}

// /llm-share 五面板页（契约 §16.3）：offer 发布 / 上游配置 / allowlist 管理 /
// borrow 快捷 / 双边账本。自持一份 ConfirmProvider（borrow 二次确认依赖，与
// main.tsx 全站 Provider 嵌套无害，acp-view 先例）。
export function LlmShareView({ backend }: { backend: LlmShareBackend }) {
  return (
    <ConfirmProvider>
      <div className="flex min-h-0 flex-col gap-4">
        <PageHeader titleKey="llmShare.title" descriptionKey="llmShare.description" />
        {/* 信息优先的动线：声明状态 → 白名单清单 → 上游配置 → 账本 → 借用动作 */}
        <div className="grid gap-6 xl:grid-cols-2">
          <PanelSection titleKey="llmShare.panels.offer" testid="section-offer">
            <OfferPanel backend={backend} />
          </PanelSection>
          <PanelSection titleKey="llmShare.panels.allowlist" testid="section-allowlist">
            <AllowlistPanel backend={backend} />
          </PanelSection>
          <PanelSection titleKey="llmShare.panels.providers" testid="section-providers">
            <ProviderPanel backend={backend} />
          </PanelSection>
          <PanelSection titleKey="llmShare.panels.ledger" testid="section-ledger">
            <LedgerPanel backend={backend} />
          </PanelSection>
          <PanelSection titleKey="llmShare.panels.borrow" testid="section-borrow">
            <BorrowPanel backend={backend} />
          </PanelSection>
        </div>
      </div>
    </ConfirmProvider>
  );
}
