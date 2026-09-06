import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useShareJoin } from "@/acp/use-share-join";

function deniedDetail(code: string | null, reason: string | null): string {
  return [code, reason].filter((v) => v !== null && v !== "").join(": ");
}

/** guest「用链接加入」卡（§8）：粘贴 dsh-acp-share:// 链接 → console
 *  POST /connect-share → 成功进连接目录（scope 徽章=分享 scope），失败显
 *  denied 码/原因。导入状态机与消息内卡片共用 useShareJoin。 */
export function ShareJoinCard() {
  const { t } = useTranslation();
  const { phase, join, reset } = useShareJoin();
  const [link, setLink] = useState("");

  const submit = () => {
    if (!link.trim()) return;
    reset();
    void join(link);
  };

  return (
    <Card data-testid="acp-share-join-card">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{t("acp.share.join.card")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <p className="text-muted-foreground text-xs">{t("acp.share.join.description")}</p>
        <div className="flex gap-2">
          <Input
            value={link}
            onChange={(e) => {
              setLink(e.target.value);
              reset();
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submit();
              }
            }}
            placeholder={t("acp.share.join.placeholder")}
            aria-label={t("acp.share.join.card")}
            autoComplete="off"
            data-testid="acp-share-join-input"
          />
          <Button
            variant="outline"
            onClick={submit}
            disabled={phase.state === "joining"}
            data-testid="acp-share-join-action"
          >
            {phase.state === "joining" ? t("acp.share.join.joining") : t("acp.share.join.action")}
          </Button>
        </div>
        {phase.state === "invalid" ? (
          <p className="text-destructive text-xs" role="alert" data-testid="acp-share-join-invalid">
            {t("acp.share.join.invalid")}
          </p>
        ) : null}
        {phase.state === "needConsole" ? (
          <p className="text-muted-foreground text-xs" data-testid="acp-share-join-need-console">
            {t("acp.share.join.needConsole")}
          </p>
        ) : null}
        {phase.state === "joined" ? (
          <p className="text-success text-xs" data-testid="acp-share-join-ok">
            {t("acp.share.join.joined")}
          </p>
        ) : null}
        {phase.state === "denied" ? (
          <p className="text-destructive text-xs" data-testid="acp-share-join-denied">
            {t("acp.share.join.denied")}
            {deniedDetail(phase.code, phase.reason) ? "：" + deniedDetail(phase.code, phase.reason) : null}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
