import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import type { DialReport, PingOutcome } from "@/lib/ipc-types";
import type { PeerEntry } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";

interface PeerRowActionsProps {
  peer: PeerEntry;
  onPing: (peer: PeerEntry) => () => Promise<PingOutcome>;
  onConnect: (peer: PeerEntry) => () => Promise<DialReport>;
  onDisconnect: (peer: PeerEntry) => () => Promise<boolean>;
  onShowDetail: (peerId: string) => void;
}

// 行操作：未连接给拨号、已连接给挂断；ping 与详情恒在（契约 §1 peer_connect/disconnect）。
export function PeerRowActions({
  peer,
  onPing,
  onConnect,
  onDisconnect,
  onShowDetail,
}: PeerRowActionsProps) {
  const { t } = useTranslation();

  return (
    <span className="flex justify-end gap-1">
      {peer.connected ? (
        <AsyncButton
          size="sm"
          variant="outline"
          action={async () => onDisconnect(peer)()}
          onSuccess={(wasConnected) => {
            if (wasConnected) toastSuccess(t("peers.disconnectOk"));
            else toastSuccess(t("peers.disconnectIdle"));
          }}
          onError={(error) => {
            const reason = errorText(error);
            toastError(t("peers.disconnectFail", { reason }), {
              description: reason,
              detail: `disconnect ${peer.peerId}: ${reason}`,
              context: "peer.disconnect",
            });
          }}
        >
          {t("common.actions.hangup")}
        </AsyncButton>
      ) : (
        <AsyncButton
          size="sm"
          variant="outline"
          action={async () => {
            const report = await onConnect(peer)();
            if (!report.ok) {
              // 兜底原因走 i18n，不展示硬编码英文。
              const failed = [...report.hops].reverse().find((hop) => !hop.ok);
              throw new Error(
                failed?.detail ?? t("peers.connectFailFallback"),
              );
            }
            return report;
          }}
          onSuccess={() => toastSuccess(t("peers.connectOk"))}
          onError={(error) => {
            const reason = errorText(error);
            toastError(t("peers.connectFail", { reason }), {
              description: reason,
              detail: `connect ${peer.peerId}: ${reason}`,
              context: "peer.connect",
            });
          }}
        >
          {t("common.actions.dial")}
        </AsyncButton>
      )}
      <AsyncButton
        size="sm"
        variant="outline"
        action={async () => {
          const outcome = await onPing(peer)();
          if (!outcome.ok) {
            throw new Error(outcome.error ?? t("peers.pingFailFallback"));
          }
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
