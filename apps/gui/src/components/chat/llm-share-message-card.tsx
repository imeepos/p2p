import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Bot } from "lucide-react";

import type { I18nKey } from "@/i18n/types";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { findLlmShareLinkInText } from "@/lib/llm-share-link-model";
import { setBorrowPrefill } from "@/views/llm-share/borrow-prefill";
import type { LlmShareBackend } from "@/views/llm-share/types";
import { errorText } from "@/views/shared/form-flow";

const REJECT_CODES = ["share-revoked", "expired", "exhausted", "bound-other", "invalid"] as const;

/** 业务拒绝码原样透出（契约 §16.6）：从错误消息中识别，未命中按通用失败处理 */
function redeemRejectCodeOf(error: unknown): string | null {
  const message = error instanceof Error ? error.message : String(error);
  return REJECT_CODES.find((code) => message.includes(code)) ?? null;
}

function rejectKeyOf(code: string): I18nKey {
  switch (code) {
    case "share-revoked":
      return "chat.llmShareMessage.codeRevoked";
    case "expired":
      return "chat.llmShareMessage.codeExpired";
    case "exhausted":
      return "chat.llmShareMessage.codeExhausted";
    case "bound-other":
      return "chat.llmShareMessage.codeBoundOther";
    default:
      return "chat.llmShareMessage.codeInvalid";
  }
}

type CardPhase =
  | { state: "idle" }
  | { state: "joining" }
  | { state: "joined" }
  | { state: "denied"; code: string | null };

// 运行时惰性解析（避免静态导入把 views/llm-share/backend → lib/ipc 拉进聊天渲染链：
// 聊天测试 vi.mock @/lib/ipc 不含 useMockIpc 导出，静态导入会使全部聊天套件崩）。
async function resolveShareBackend(): Promise<LlmShareBackend> {
  const mod = await import("@/views/llm-share/backend");
  return mod.resolveLlmShareBackend();
}

// 聊天消息内的 llm-share 分享链接卡片（scheme dsh-llm-share://，独立域不复用 ACP）：
// 点击 → shareRedeem(link)（借方拨号兑换）→ 成功 toast + 跳 /llm-share?peer=&model=
// 预填借用表单；业务拒绝码原样透出不本地化改写。backend 缺省运行时解析（测试可注入）。
export function LlmShareMessageCard({ link, backend }: { link: string; backend?: LlmShareBackend }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [phase, setPhase] = useState<CardPhase>({ state: "idle" });

  const join = async () => {
    setPhase({ state: "joining" });
    try {
      const resolved = backend ?? (await resolveShareBackend());
      const result = await resolved.shareRedeem(link);
      const model = result.offer.models[0] ?? "";
      // 借出方 offer 快照（redeem 应答内嵌）→ borrow 预填：targetPeer/model/模型候选源
      setBorrowPrefill({ peer: result.offer.peer, model, models: result.offer.models });
      setPhase({ state: "joined" });
      toastSuccess(t("chat.llmShareMessage.redeemed"));
      navigate(`/llm-share?peer=${encodeURIComponent(result.offer.peer)}&model=${encodeURIComponent(model)}`);
    } catch (error) {
      console.error("[chat] llm-share 链接兑换失败", error);
      const code = redeemRejectCodeOf(error);
      setPhase({ state: "denied", code });
      toastError(t("chat.llmShareMessage.denied"), {
        description: code ? t(rejectKeyOf(code)) : errorText(error),
      });
    }
  };

  return (
    <div className="flex flex-col gap-1">
      <div
        className="bg-background text-foreground flex items-center gap-2 rounded-md border px-2.5 py-2"
        data-testid="chat-llm-share-card"
      >
        <Bot aria-hidden className="text-muted-foreground size-4 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium">{t("chat.llmShareMessage.title")}</p>
          <p className="text-muted-foreground truncate text-[10px]">
            {phase.state === "joined"
              ? t("chat.llmShareMessage.joined")
              : t("chat.llmShareMessage.hint")}
          </p>
        </div>
        {phase.state !== "joined" ? (
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 shrink-0 px-2 text-xs"
            onClick={() => void join()}
            disabled={phase.state === "joining"}
            data-testid="chat-llm-share-join"
          >
            {phase.state === "joining"
              ? t("chat.llmShareMessage.joining")
              : t("chat.llmShareMessage.action")}
          </Button>
        ) : null}
      </div>
      {phase.state === "denied" ? (
        <p className="text-xs opacity-80" data-testid="chat-llm-share-denied">
          {t("chat.llmShareMessage.denied")}
          {phase.code ? "：" + t(rejectKeyOf(phase.code)) : ""}
        </p>
      ) : null}
    </div>
  );
}

/** llm-share 文本渲染入口：识别 dsh-llm-share:// 链接为卡片，其余文字保留 */
export function TextWithLlmShareLink({ text, backend }: { text: string; backend?: LlmShareBackend }) {
  const link = findLlmShareLinkInText(text);
  if (!link) return <p className="whitespace-pre-wrap break-words">{text}</p>;
  const index = text.indexOf(link);
  const before = text.slice(0, index).trim();
  const after = text.slice(index + link.length).trim();
  return (
    <div className="flex flex-col gap-1.5">
      {before ? <p className="whitespace-pre-wrap break-words">{before}</p> : null}
      <LlmShareMessageCard link={link} backend={backend} />
      {after ? <p className="whitespace-pre-wrap break-words">{after}</p> : null}
    </div>
  );
}