import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { MessageSquare } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
import { ConversationList } from "@/components/chat/conversation-list";
import { Composer } from "@/components/chat/composer";
import { MessageList } from "@/components/chat/message-list";
import { useChatStore } from "@/stores/chat-store";
import { EmptyState } from "@/views/shared/empty-state";

export function ChatView() {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const friendsLoaded = useChatStore((s) => s.friendsLoaded);
  const friendsError = useChatStore((s) => s.friendsError);
  const selectedPeer = useChatStore((s) => s.selectedPeer);
  const messagesByPeer = useChatStore((s) => s.messagesByPeer);
  const lastMessages = useChatStore((s) => s.lastMessageByPeer);
  const historyLoadingAll = useChatStore((s) => s.historyLoading);
  const hasMoreAll = useChatStore((s) => s.hasMore);
  const messages = selectedPeer ? messagesByPeer[selectedPeer] ?? [] : [];
  const historyLoading = selectedPeer ? historyLoadingAll[selectedPeer] ?? false : false;
  const hasMore = selectedPeer ? hasMoreAll[selectedPeer] ?? false : false;
  const loadFriends = useChatStore((s) => s.loadFriends);
  const selectPeer = useChatStore((s) => s.selectPeer);
  const loadOlder = useChatStore((s) => s.loadOlder);
  const cancelPending = useChatStore((s) => s.cancelPending);
  const subscribeEvents = useChatStore((s) => s.subscribeEvents);

  useEffect(() => {
    void loadFriends();
    void subscribeEvents();
  }, [loadFriends, subscribeEvents]);

  const selectedFriend = friends.find((f) => f.peerId === selectedPeer);

  return (
    <>
      <PageHeader titleKey="chat.title" descriptionKey="chat.description" />
      <div className="grid min-h-72 grid-cols-[16rem_1fr] gap-4">
        <section aria-label={t("chat.friends")} className="rounded-lg border">
          <h2 className="px-3 py-2 font-medium">{t("chat.friends")}</h2>
          <ConversationList
            friends={friends}
            lastMessages={lastMessages}
            selectedPeer={selectedPeer}
            loading={!friendsLoaded && !friendsError}
            error={friendsError}
            onSelect={(peerId) => void selectPeer(peerId)}
          />
        </section>

        <section
          aria-label={t("chat.conversation")}
          className="flex min-h-72 flex-col rounded-lg border"
        >
          {selectedPeer ? (
            <>
              <div className="border-b px-4 py-2">
                <div className="text-sm font-medium">
                  {selectedFriend?.nickname || selectedPeer.slice(0, 8)}
                </div>
                <div className="text-xs text-muted-foreground">
                  {selectedPeer}
                </div>
              </div>
              <MessageList
                peer={selectedPeer}
                messages={messages}
                loadingOlder={historyLoading}
                hasMore={hasMore}
                onLoadOlder={() => void loadOlder(selectedPeer)}
                onCancelPending={(id) => cancelPending(selectedPeer, id)}
              />
              <Composer peer={selectedPeer} />
            </>
          ) : (
            <EmptyState
              className="flex-1"
              icon={MessageSquare}
              title={t("chat.conversationEmpty")}
              description={t("chat.noFriendsHint")}
            />
          )}
        </section>
      </div>
    </>
  );
}
