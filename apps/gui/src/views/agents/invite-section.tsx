// 私密邀请节（design §8.2 Q2）：收到的待处理邀请（accept/reject）+ 已发送的待回执。
import { useTranslation } from "react-i18next";
import { Mail, Send, Check, X, Clock } from "lucide-react";

import { Button } from "@/components/ui/button";
import { SectionHeader } from "@/views/shared/section-header";

import type { InviteEntryJson } from "@/a2a/types";

interface InviteSectionProps {
  received: InviteEntryJson[];
  sent: InviteEntryJson[];
  onAccept: (nonce: string) => Promise<void>;
  onReject: (nonce: string) => Promise<void>;
}

export function InviteSection({ received, sent, onAccept, onReject }: InviteSectionProps) {
  const { t } = useTranslation();
  const pendingReceived = received.filter((i) => i.status === "pending");
  const pendingSent = sent.filter((i) => i.status === "pending");

  return (
    <section data-testid="agents-invite-section" className="flex flex-col gap-4">
      {/* 收到的邀请 */}
      <div className="flex flex-col gap-2">
        <SectionHeader
          icon={Mail}
          title={t("agents.section.invitesReceived")}
          tone="info"
          count={pendingReceived.length}
        />
        {pendingReceived.length === 0 ? (
          <p className="text-muted-foreground rounded-md border border-dashed p-4 text-center text-sm" data-testid="agents-invites-received-empty">
            {t("agents.empty.invitesReceived")}
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {pendingReceived.map((invite) => (
              <li
                key={invite.nonce}
                data-testid="agents-invite-received-row"
                className="bg-card ring-border flex flex-col gap-1 rounded-lg p-3 ring-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium">{invite.agentId}</span>
                  <span className="text-muted-foreground text-xs">
                    {t("agents.invite.from")}: {invite.hostPeer.slice(0, 12)}...
                  </span>
                  <span className="ml-auto flex items-center gap-1">
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="text-green-600 gap-1 px-2 hover:text-green-600"
                      onClick={() => onAccept(invite.nonce)}
                      data-testid="agents-invite-accept-btn"
                    >
                      <Check aria-hidden className="size-4" />
                      {t("agents.action.accept")}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="text-destructive gap-1 px-2 hover:text-destructive"
                      onClick={() => onReject(invite.nonce)}
                      data-testid="agents-invite-reject-btn"
                    >
                      <X aria-hidden className="size-4" />
                      {t("agents.action.reject")}
                    </Button>
                  </span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* 已发送的邀请 */}
      <div className="flex flex-col gap-2">
        <SectionHeader
          icon={Send}
          title={t("agents.section.invitesSent")}
          tone="primary"
          count={pendingSent.length}
        />
        {pendingSent.length === 0 ? (
          <p className="text-muted-foreground rounded-md border border-dashed p-4 text-center text-sm" data-testid="agents-invites-sent-empty">
            {t("agents.empty.invitesSent")}
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {pendingSent.map((invite) => (
              <li
                key={invite.nonce}
                data-testid="agents-invite-sent-row"
                className="bg-card ring-border flex flex-col gap-1 rounded-lg p-3 ring-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium">{invite.agentId}</span>
                  <span className="text-muted-foreground text-xs">
                    {t("agents.invite.to")}: {invite.inviteePeer.slice(0, 12)}...
                  </span>
                  <span className="ml-auto flex items-center gap-1 text-muted-foreground">
                    <Clock aria-hidden className="size-4" />
                    <span className="text-xs">
                      {invite.status === "pending"
                        ? t("agents.invite.pending")
                        : invite.status === "accepted"
                          ? t("agents.invite.accepted")
                          : t("agents.invite.rejected")}
                    </span>
                  </span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
