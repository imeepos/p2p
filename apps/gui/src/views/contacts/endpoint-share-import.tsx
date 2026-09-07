import { useState } from "react";
import { useTranslation } from "react-i18next";
import { CircleCheck, Loader2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/feedback/copy-button";
import { useShareJoin, type ShareJoinConnection } from "@/acp/use-share-join";

/** 分享链接导入区（§8 guest 导入的弹窗形态）：wsUrl 字段识别到
 *  dsh-acp-share:// 链接时替换手工表单；成功经 onJoined 落 saved
 *  endpoint（幂等 id，重导入即刷新），失败显式呈现 denied 码/原因。 */
export function EndpointShareImport({ link, conn, onJoined, onExit }: {
  link: string;
  conn: ShareJoinConnection | null;
  onJoined: (peer: string) => void;
  onExit: () => void;
}) {
  const { t } = useTranslation();
  const { phase, join, reset } = useShareJoin();
  const [denyNote, setDenyNote] = useState<string | null>(null);

  const submit = () => {
    setDenyNote(null);
    void join(link, conn ?? undefined)
      .then((result) => {
        if (result.state === "joined") {
          onJoined(result.peer);
        } else if (result.state === "denied") {
          setDenyNote(
            [result.code, result.reason].filter((v) => v !== null && v !== "").join(": ") || null,
          );
        }
      })
      .catch((error) => {
        // 失败路径留可观测信号（console + 行内 denied 面），不静默吞
        console.error("[contacts] 分享链接导入失败", error);
        setDenyNote(String(error));
      });
  };

  const joining = phase.state === "joining";
  return (
    <div className="flex flex-col gap-2" data-testid="contacts-endpoint-share-import">
      <p className="text-muted-foreground text-xs">{t("contacts.endpoint.shareImportHint")}</p>
      <p className="font-mono text-xs break-all" data-testid="contacts-endpoint-share-import-link">{link}</p>
      <div className="flex items-center gap-2">
        <Button type="button" size="sm" onClick={submit} disabled={joining} data-testid="contacts-endpoint-share-import-action">
          {joining ? (
            <>
              <Loader2Icon aria-hidden className="size-4 animate-spin" />
              {t("contacts.endpoint.shareImporting")}
            </>
          ) : (
            t("contacts.endpoint.shareImport")
          )}
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={() => { reset(); onExit(); }} data-testid="contacts-endpoint-share-import-back">
          {t("contacts.endpoint.shareImportBack")}
        </Button>
      </div>
      {phase.state === "joined" ? (
        <span className="inline-flex items-center gap-1 text-success text-xs" data-testid="contacts-endpoint-share-import-ok">
          <CircleCheck aria-hidden className="size-3.5" />
          {t("contacts.endpoint.shareImported")}
        </span>
      ) : null}
      {phase.state === "invalid" ? (
        <p className="text-destructive text-xs" data-testid="contacts-endpoint-share-import-invalid">
          {t("contacts.endpoint.shareImportInvalid")}
        </p>
      ) : null}
      {phase.state === "needConsole" ? (
        <p className="text-muted-foreground text-xs" data-testid="contacts-endpoint-share-import-need-console">
          {t("contacts.endpoint.shareImportNeedConsole")}
        </p>
      ) : null}
      {phase.state === "denied" ? (
        <div className="flex items-center gap-1" data-testid="contacts-endpoint-share-import-denied-row">
          <p className="text-destructive text-xs" data-testid="contacts-endpoint-share-import-denied">
            {t("contacts.endpoint.shareImportDenied")}
            {denyNote ? "：" + denyNote : null}
          </p>
          <CopyButton
            value={t("contacts.endpoint.shareImportDenied") + (denyNote ? "：" + denyNote : "")}
            className="size-5"
          />
        </div>
      ) : null}
    </div>
  );
}