import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

interface PeerStatusDotProps {
  online: boolean;
  testId?: string;
  withLabel?: boolean;
}

// 纯展示圆点：绿=在线灰=离线；在线态由调用方经 usePeerOnline 注入。
export function PeerStatusDot({ online, testId, withLabel = false }: PeerStatusDotProps) {
  const { t } = useTranslation();
  const label = online ? t("chat.peerOnline") : t("chat.peerOffline");
  return (
    <span className="inline-flex items-center gap-1.5">
      <span
        data-testid={testId}
        data-online={online ? "true" : "false"}
        title={label}
        aria-label={label}
        className={cn(
          "size-2 shrink-0 rounded-full",
          online ? "bg-success" : "bg-muted-foreground/40",
        )}
      />
      {withLabel ? (
        <span className="text-xs font-normal text-muted-foreground">{label}</span>
      ) : null}
    </span>
  );
}
