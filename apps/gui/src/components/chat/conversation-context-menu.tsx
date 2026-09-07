import { useTranslation } from "react-i18next";

import { useAcpStore } from "@/acp/acp-store";
import {
  ContextMenu,
  ContextMenuItem,
  ContextMenuSeparator,
  type ContextMenuAnchor,
} from "@/components/ui/context-menu";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { openConversationWindow } from "@/lib/open-conversation-window";
import { useChatStore } from "@/stores/chat-store";
import {
  conversationKey,
  useConversationPrefsStore,
} from "@/stores/conversation-prefs-store";
import { useGroupStore } from "@/stores/group-store";

// 会话列表右键菜单（微信同款六项）：置顶/标为未读/消息免打扰/独立窗口/不显示/删除。
// 全部为本地偏好与会话列表层语义：删除=隐藏列表项并清旗标，不删好友、不退群、
// 不动历史；晚于该时点的新消息会让会话回归列表。

interface ConversationContextMenuProps {
  entry: ConversationEntry | null;
  anchor: ContextMenuAnchor | null;
  onClose: () => void;
}

export function ConversationContextMenu({ entry, anchor, onClose }: ConversationContextMenuProps) {
  const { t } = useTranslation();
  const flags = useConversationPrefsStore((s) =>
    entry ? s.flags[conversationKey(entry.kind, entry.id)] : undefined,
  );
  if (!entry) {
    return <ContextMenu anchor={null} label="" onClose={onClose}>{null}</ContextMenu>;
  }
  const key = conversationKey(entry.kind, entry.id);
  const unread = (flags?.manualUnread ?? false) || entry.unread > 0;
  const store = useConversationPrefsStore.getState;
  const markRead = () => {
    store().setManualUnread(key, false);
    if (entry.kind === "friend") useChatStore.getState().markPeerRead(entry.id);
    else if (entry.kind === "group") useGroupStore.getState().markGroupRead(entry.id);
    else useAcpStore.getState().markEndpointRead(entry.id);
  };
  return (
    <ContextMenu
      anchor={anchor}
      label={t("chat.conversations.contextMenu.menuLabel")}
      onClose={onClose}
      testId="conversation-context-menu"
    >
      <ContextMenuItem
        testId="conversation-menu-pin"
        onSelect={() => store().togglePinned(key)}
      >
        {flags?.pinned ? t("chat.conversations.contextMenu.unpin") : t("chat.conversations.contextMenu.pin")}
      </ContextMenuItem>
      <ContextMenuItem
        testId="conversation-menu-unread"
        onSelect={() => (unread ? markRead() : store().setManualUnread(key, true))}
      >
        {unread
          ? t("chat.conversations.contextMenu.markRead")
          : t("chat.conversations.contextMenu.markUnread")}
      </ContextMenuItem>
      <ContextMenuItem testId="conversation-menu-mute" onSelect={() => store().toggleMuted(key)}>
        {t("chat.conversations.contextMenu.mute")}
      </ContextMenuItem>
      <ContextMenuItem
        testId="conversation-menu-window"
        onSelect={() => void openConversationWindow(entry)}
      >
        {t("chat.conversations.contextMenu.openWindow")}
      </ContextMenuItem>
      <ContextMenuItem
        testId="conversation-menu-hide"
        onSelect={() => store().hideConversation(key, Date.now())}
      >
        {t("chat.conversations.contextMenu.hide")}
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        testId="conversation-menu-delete"
        destructive
        onSelect={() => store().removeConversation(key, Date.now())}
      >
        {t("chat.conversations.contextMenu.remove")}
      </ContextMenuItem>
    </ContextMenu>
  );
}
