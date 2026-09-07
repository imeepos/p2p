import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Settings2 } from "lucide-react";

import { Composer, type ComposerTransport } from "@/components/chat/composer";
import type { BubbleAvatar } from "@/components/chat/message-bubble";
import { NodeStoppedCard } from "@/components/chat/node-stopped-card";
import { Button } from "@/components/ui/button";
import type { ChatMessageJson, GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { useNodeStore } from "@/stores/node-store";
import { useProfileStore } from "@/stores/profile-store";

import { GroupMessageList } from "./group-message-list";
import { GroupStateBadge } from "./group-list";
import { toBubbleMessage } from "./group-names";

interface GroupConversationProps {
  group: GroupJson;
  onOpenManage: () => void;
}

// 群会话右栏：头部（名/状态徽标/成员数/管理入口）+ 消息流 + 输入条。
// 非 active 群消息只读（设计 §5：退群/被踢/解散后不可再发，历史保留）；
// 输入条复用 1:1 Composer，经 transport 注入群发送面。
export function GroupConversation({ group, onOpenManage }: GroupConversationProps) {
  const { t } = useTranslation();
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const friends = useGroupStore((s) => s.friends);
  const messagesByGroup = useGroupStore((s) => s.messagesByGroup);
  const historyLoadingAll = useGroupStore((s) => s.historyLoading);
  const hasMoreAll = useGroupStore((s) => s.hasMore);
  const historyErrorAll = useGroupStore((s) => s.historyError);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const loadOlder = useGroupStore((s) => s.loadOlder);
  const cancelPending = useGroupStore((s) => s.cancelPending);
  const sendText = useGroupStore((s) => s.sendText);
  const sendMedia = useGroupStore((s) => s.sendMedia);
  const [replyTarget, setReplyTarget] = useState<ChatMessageJson | null>(null);
  const profile = useProfileStore((s) => s.profile);

  const selfAvatar: BubbleAvatar = {
    label: profile.name.trim() || "P2P",
    seed: "self",
    src: profile.avatar,
  };

  const nodeStatus = useNodeStore((s) => s.status);
  const nodeStopped = nodeStatus !== null && !nodeStatus.running;
  const readOnly = group.state !== "active";

  const messages = messagesByGroup[group.groupId] ?? [];
  const totalRecipients = group.members.filter((m) => m !== selfPeerId).length;

  const transport = useMemo<ComposerTransport>(
    () => ({
      sendText: (groupId, text, replyTo) => sendText(groupId, text, replyTo),
      sendMedia: (groupId, kind, media, replyTo) => sendMedia(groupId, kind, media, replyTo),
    }),
    [sendText, sendMedia],
  );

  const onReply = (message: ChatMessageJson) => setReplyTarget(message);

  return (
    <>
      <div
        data-testid="group-conversation-header"
        className="relative flex h-12 shrink-0 items-center border-b border-border/60 px-4"
      >
        <div className="absolute left-1/2 flex min-w-0 max-w-[60%] -translate-x-1/2 items-center gap-2 text-sm font-medium">
          <span className="truncate">{group.name}</span>
          {group.state !== "active" ? <GroupStateBadge state={group.state} /> : null}
          <span className="text-muted-foreground shrink-0 text-xs">
            {t("group.members", { count: group.members.length })}
          </span>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-muted-foreground hover:text-foreground ml-auto"
          data-testid="group-manage"
          onClick={onOpenManage}
        >
          <Settings2 aria-hidden />
          {t("group.manage.action")}
        </Button>
      </div>
      {readOnly ? (
        <p
          data-testid="group-readonly-hint"
          className="shrink-0 bg-muted/50 px-4 py-1.5 text-xs text-muted-foreground"
        >
          {t("group.readOnlyHint", { state: t(`group.state.${group.state}`) })}
        </p>
      ) : null}
      <GroupMessageList
        groupId={group.groupId}
        messages={messages}
        selfPeerId={selfPeerId}
        friends={friends}
        totalRecipients={totalRecipients}
        loadingOlder={historyLoadingAll[group.groupId] ?? false}
        hasMore={hasMoreAll[group.groupId] ?? false}
        historyError={historyErrorAll[group.groupId] ?? null}
        onLoadOlder={() => void loadOlder(group.groupId)}
        onRetryHistory={() => selectGroup(group.groupId)}
        onCancelPending={(messageId) => cancelPending(group.groupId, messageId)}
        onReply={
          readOnly
            ? undefined
            : (message) => onReply(toBubbleMessage(message, selfPeerId))
        }
        selfAvatar={selfAvatar}
      />
      {nodeStopped ? <NodeStoppedCard /> : null}
      <Composer
        peer={group.groupId}
        replyTarget={replyTarget}
        onReplyCancel={() => setReplyTarget(null)}
        disabled={nodeStopped || readOnly}
        transport={transport}
        testIds={{ input: "group-input", send: "group-send" }}
      />
    </>
  );
}
