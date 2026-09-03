import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import type { PingOutcome } from "@/lib/ipc-types";
import type { PeerEntry } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";

interface PeerRowActionsProps {
  peer: PeerEntry;
  onPing: (peer: PeerEntry) => () => Promise<PingOutcome>;
  onShowDetail: (peerId: string) => void;
}

// 行操作：ping（结果 toast + rtt，超时按失败态）与详情抽屉入口。
export function PeerRowActions({
  peer,
  onPing,
  onShowDetail,
}: PeerRowActionsProps) {
  const { t } = useTranslation();

  return (
    <span className="flex justify-end gap-1">
      <AsyncButton
        size="sm"
        variant="outline"
        action={async () => {
          const outcome = await onPing(peer)();
          if (!outcome.ok) throw new Error(outcome.error ?? "ping failed");
          return outcome;
        }}
        onSuccess={(result) =>
          toastSuccess(
            t("peers.pingOk", { rtt: (result as PingOutcome).rttMs }),
          )
        }
        onError={(error) => {
          const reason = errorText(error);
          toastError(t("peers.pingFail", { reason }), {
            description: reason,
            detail: `ping ${peer.peerId}: ${reason}`,
            context: "peer.ping",
          });
        }}
      >
        {t("common.actions.ping")}
      </AsyncButton>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => onShowDetail(peer.peerId)}
      >
        {t("common.actions.detail")}
      </Button>
    </span>
  );
}
