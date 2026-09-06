import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { UsersRound } from "lucide-react";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import { formatTime } from "@/lib/format";
import type { GroupInviteJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import type { Locale } from "@/i18n";
import { EmptyState } from "@/views/shared/empty-state";

// 入群邀请列表（IMC3 需求 2）：每条含方向/状态徽章/时间/备注；in 向待处理
// 行内同意/拒绝；行点击跳对应群会话（roster 未达由会话页加载态兜底）；
// 操作失败原文上浮不静默。

function DirectionBadge({ direction }: { direction: GroupInviteJson["direction"] }) {
  const { t } = useTranslation();
  return (
    <span className="bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-xs">
      {t(direction === "in" ? "messages.direction.in" : "messages.direction.out")}
    </span>
  );
}

function StateBadge({ state }: { state: GroupInviteJson["state"] }) {
  const { t } = useTranslation();
  const tone =
    state === "accepted"
      ? "text-primary"
      : state === "rejected"
        ? "text-muted-foreground"
        : "text-secondary-foreground bg-secondary";
  return (
    <span
      data-testid={"group-invite-row-state"}
      className={
        "rounded-full px-2 py-0.5 text-xs font-medium " +
        (state === "pending" ? tone : "bg-muted " + tone)
      }
    >
      {t(
        state === "pending"
          ? "messages.state.pending"
          : state === "accepted"
            ? "messages.state.accepted"
            : "messages.state.rejected",
      )}
    </span>
  );
}

export function GroupInviteSection() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const navigate = useNavigate();
  const invites = useChatStore((s) => s.groupInvites);
  const listError = useChatStore((s) => s.groupInvitesError);
  const acceptGroupInvite = useChatStore((s) => s.acceptGroupInvite);
  const rejectGroupInvite = useChatStore((s) => s.rejectGroupInvite);
  const [rowErrors, setRowErrors] = useState<Record<string, string>>({});

  const rows = [...invites].sort((a, b) => b.tsMs - a.tsMs);

  const act = async (invite: GroupInviteJson, kind: "accept" | "reject") => {
    setRowErrors((prev) => ({ ...prev, [invite.id]: "" }));
    try {
      if (kind === "accept") {
        await acceptGroupInvite(invite.id);
      } else {
        await rejectGroupInvite(invite.id, null);
      }
    } catch (err) {
      console.error("[messages] 入群邀请行内操作失败", invite.id, err);
      const detail = err instanceof Error ? err.message : String(err);
      setRowErrors((prev) => ({
        ...prev,
        [invite.id]:
          t(kind === "accept" ? "messages.error.acceptFailed" : "messages.error.rejectFailed") +
          detail,
      }));
    }
  };

  return (
    <section data-testid="messages-group-section" className="flex flex-col gap-2">
      <h2 className="text-sm font-semibold">{t("messages.section.groups")}</h2>
      {listError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="messages-group-list-error">
          {t("messages.error.listLoadFailed") + listError}
        </p>
      ) : null}
      {rows.length === 0 && !listError ? (
        <EmptyState icon={UsersRound} title={t("messages.empty.groups")} />
      ) : null}
      <div className="flex flex-col gap-2">
        {rows.map((invite) => {
          const actionable = invite.direction === "in" && invite.state === "pending";
          return (
            <div
              key={invite.id}
              data-testid={"messages-group-row-" + invite.id}
              className="bg-card ring-border ring-1 hover:ring-ring cursor-pointer rounded-lg p-3 transition-shadow"
              onClick={() => navigate("/chat?group=" + invite.groupId)}
            >
              <div className="flex flex-wrap items-center gap-2">
                <DirectionBadge direction={invite.direction} />
                <span className="text-sm font-medium">{invite.groupName}</span>
                <span className="ml-auto flex items-center gap-2">
                  {actionable ? (
                    <>
                      <AsyncButton
                        type="button"
                        size="sm"
                        action={() => act(invite, "accept")}
                        data-testid={"messages-group-accept-" + invite.id}
                      >
                        {t("messages.action.accept")}
                      </AsyncButton>
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        onClick={(event) => {
                          event.stopPropagation();
                          void act(invite, "reject");
                        }}
                        data-testid={"messages-group-reject-" + invite.id}
                      >
                        {t("messages.action.reject")}
                      </Button>
                    </>
                  ) : null}
                  <StateBadge state={invite.state} />
                  <time className="text-muted-foreground text-xs">
                    {formatTime(invite.tsMs, locale)}
                  </time>
                </span>
              </div>
              <p className="text-muted-foreground mt-1 font-mono text-xs">
                {invite.groupId}
              </p>
              {invite.note ? (
                <p className="text-foreground/80 mt-1 text-xs">
                  {t("chat.groupInvite.noteLabel", { note: invite.note })}
                </p>
              ) : null}
              {rowErrors[invite.id] ? (
                <p className="text-destructive mt-1 text-xs" role="alert">
                  {rowErrors[invite.id]}
                </p>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}
