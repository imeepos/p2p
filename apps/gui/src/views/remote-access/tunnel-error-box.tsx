import { useTranslation } from "react-i18next";

// 操作失败错误盒（role=alert 既有盒样式）：最近错误 + 可读原因，
// 可选节点离线提示。三处卡片共用，避免盒样式各写一套。
export function TunnelErrorBox({
  message,
  showNodeHint = false,
  testId,
}: {
  message: string;
  showNodeHint?: boolean;
  testId?: string;
}) {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      className="text-destructive space-y-1 rounded-md border border-red-300 p-3 text-sm"
    >
      <p className="font-medium">{t("remoteAccess.error.lastError")}</p>
      <p data-testid={testId}>{message}</p>
      {showNodeHint ? (
        <p className="text-muted-foreground">
          {t("remoteAccess.error.nodeOfflineHint")}
        </p>
      ) : null}
    </div>
  );
}
