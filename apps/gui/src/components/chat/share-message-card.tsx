import { useTranslation } from "react-i18next";
import { Bot } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useShareJoin } from "@/acp/use-share-join";
import { findShareLinkInText } from "@/acp/share-model";

import { RichTextMessage } from "./rich-text-message";

// 聊天消息内的分享链接卡片（acp-share §8）：transcript 渲染层识别，
// 纯展示，不改 ChatEnvelope/线协议；点击与「用链接加入」走同一导入路径。
export function ShareMessageCard({ link }: { link: string }) {
  const { t } = useTranslation();
  const { phase, join, reset } = useShareJoin();

  const action = () => {
    reset();
    void join(link);
  };

  return (
    <div className="flex flex-col gap-1">
      <div
        className="bg-background text-foreground flex items-center gap-2 rounded-md border px-2.5 py-2"
        data-testid="chat-share-link-card"
      >
        <Bot aria-hidden className="text-muted-foreground size-4 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium">{t("acp.share.message.title")}</p>
          <p className="text-muted-foreground truncate text-[10px]">
            {phase.state === "joined" ? t("acp.share.message.joined") : t("acp.share.message.hint")}
          </p>
        </div>
        {phase.state !== "joined" ? (
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 shrink-0 px-2 text-xs"
            onClick={action}
            disabled={phase.state === "joining"}
            data-testid="chat-share-link-join"
          >
            {phase.state === "joining" ? t("acp.share.message.joining") : t("acp.share.message.action")}
          </Button>
        ) : null}
      </div>
      {phase.state === "invalid" ? (
        <p className="text-xs opacity-80" data-testid="chat-share-link-invalid">
          {t("acp.share.join.invalid")}
        </p>
      ) : null}
      {phase.state === "needConsole" ? (
        <p className="text-xs opacity-80" data-testid="chat-share-link-need-console">
          {t("acp.share.message.needConsole")}
        </p>
      ) : null}
      {phase.state === "denied" ? (
        <p className="text-xs opacity-80" data-testid="chat-share-link-denied">
          {t("acp.share.message.denied")}
          {phase.code ? "：" + phase.code : ""}
        </p>
      ) : null}
    </div>
  );
}

/** 文本消息渲染入口：无链接走富文本渲染；含链接时链接被识别为卡片，其余文字富文本保留 */
export function TextWithShareLink({ text }: { text: string }) {
  const link = findShareLinkInText(text);
  if (!link) return <RichTextMessage text={text} />;
  const index = text.indexOf(link);
  const before = text.slice(0, index).trim();
  const after = text.slice(index + link.length).trim();
  return (
    <div className="flex flex-col gap-1.5">
      {before ? <RichTextMessage text={before} /> : null}
      <ShareMessageCard link={link} />
      {after ? <RichTextMessage text={after} /> : null}
    </div>
  );
}
