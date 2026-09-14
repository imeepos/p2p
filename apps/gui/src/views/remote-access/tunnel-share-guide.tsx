import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { copyText } from "@/views/shared/clipboard";

import { cliConnectCommand, targetPort } from "./tunnel-flow";

// 对端使用说明（gap-matrix §4.1-5）：serve 开放成功后生成，含 GUI 路径
// （端口 + 本机 PeerId）与 CLI 命令原文，一键复制全量文本。
export function TunnelShareGuide({
  selfPeerId,
  target,
}: {
  selfPeerId: string | null;
  target: string;
}) {
  const { t } = useTranslation();
  const port = targetPort(target);
  const cliCommand = selfPeerId
    ? cliConnectCommand(selfPeerId, target)
    : null;
  const guiLine = t("remoteAccess.tunnel.guideGuiLine", {
    port,
    peer: selfPeerId ?? t("remoteAccess.tunnel.guideSelfPeerMissing"),
  });
  const fullText = cliCommand
    ? guiLine + "\n" + t("remoteAccess.tunnel.guideCliLine", { cmd: cliCommand })
    : guiLine;
  return (
    <div
      className="space-y-2 rounded-md border border-dashed p-3 text-sm"
      data-testid="tunnel-share-guide"
    >
      <p className="font-medium">{t("remoteAccess.tunnel.guideTitle")}</p>
      <p>{guiLine}</p>
      {cliCommand ? (
        <p className="font-mono break-all" data-testid="tunnel-share-guide-cli">
          {t("remoteAccess.tunnel.guideCliLine", { cmd: cliCommand })}
        </p>
      ) : (
        <p className="text-muted-foreground">
          {t("remoteAccess.tunnel.guideSelfPeerMissing")}
        </p>
      )}
      <Button
        type="button"
        size="sm"
        variant="outline"
        data-testid="tunnel-share-guide-copy"
        onClick={() => {
          void copyText(fullText, {
            done: t("remoteAccess.tunnel.guideCopied"),
            failed: t("remoteAccess.tunnel.copyFailed"),
          });
        }}
      >
        {t("remoteAccess.tunnel.guideCopy")}
      </Button>
    </div>
  );
}
