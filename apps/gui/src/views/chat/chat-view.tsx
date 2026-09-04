import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { MessageSquare, UserPlusIcon } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
import { Button } from "@/components/ui/button";
import { ChatFriendAddDialog } from "@/components/chat/chat-friend-add-dialog";
import { ChatFriendRemoveDialog } from "@/components/chat/chat-friend-remove-dialog";
import { ConversationList } from "@/components/chat/conversation-list";
import { PeerStatusDot } from "@/components/chat/peer-status";
import type { ChatFriendJson, ChatMessageJson } from "@/lib/ipc-types";
import { Composer } from "@/components/chat/composer";
import { MessageList } from "@/components/chat/message-list";
import { useChatStore } from "@/stores/chat-store";
import { usePeerOnline } from "@/stores/node-store";
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
  const [addFriendOpen, setAddFriendOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ChatFriendJson | null>(null);
  // 回复引用预览（IM-T46B）：页内状态即可（Composer/MessageList 同树）；
  // 清空走 onSelect 事件路径与发送成功回调，避免把 A 会话的引用带进 B 会话发送。
  const [replyTarget, setReplyTarget] = useState<ChatMessageJson | null>(null);

  useEffect(() => {
    void loadFriends();
    void subscribeEvents();
  }, [loadFriends, subscribeEvents]);

  const selectedFriend = friends.find((f) => f.peerId === selectedPeer);
  const selectedOnline = usePeerOnline(selectedPeer ?? "");

  return (
    <>
      <PageHeader titleKey="chat.title" descriptionKey="chat.description" />
      <div className="col-span-12 grid min-h-[calc(100vh-220px)] grid-cols-[16rem_1fr] gap-4">
        <section aria-label={t("chat.friends")} className="flex flex-col rounded-lg border">
          <div className="flex items-center justify-between gap-2 px-3 py-2">
            <h2 className="font-medium">{t("chat.friends")}</h2>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setAddFriendOpen(true)}
              data-testid="chat-add-friend"
            >
              <UserPlusIcon aria-hidden />
              {t("chat.addFriend.action")}
            </Button>
          </div>
          <ConversationList
            friends={friends}
            lastMessages={lastMessages}
            selectedPeer={selectedPeer}
            loading={!friendsLoaded && !friendsError}
            error={friendsError}
            onSelect={(peerId) => {
              // 切会话即弃引用预览：A 会话的引用不得带进 B 会话发送
              setReplyTarget(null);
              void selectPeer(peerId);
            }}
            onAddFriend={() => setAddFriendOpen(true)}
            onRemoveFriend={(peerId) =>
              setRemoveTarget(friends.find((f) => f.peerId === peerId) ?? null)
            }
            onRetry={async () => {
              await loadFriends();
              const err = useChatStore.getState().friendsError;
              if (err) throw new Error(err);
            }}
          />
        </section>

        <section
          aria-label={t("chat.conversation")}
          className="flex min-h-72 flex-col rounded-lg border"
        >
          {selectedPeer ? (
            <>
              <div className="border-b px-4 py-2">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <span>{selectedFriend?.nickname || selectedPeer.slice(0, 8)}</span>
                  <PeerStatusDot
                    online={selectedOnline}
                    testId="chat-header-status"
                    withLabel
                  />
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
                onReply={setReplyTarget}
              />
              <Composer
                peer={selectedPeer}
                replyTarget={replyTarget}
                onReplyCancel={() => setReplyTarget(null)}
              />
            </>
          ) : (
            <EmptyState
              className="max-w-none flex-1"
              icon={MessageSquare}
              title={t("chat.conversationEmpty")}
              description={t("chat.noFriendsHint")}
            />
          )}
        </section>
      </div>
      <ChatFriendAddDialog open={addFriendOpen} onOpenChange={setAddFriendOpen} />
      {removeTarget ? (
        <ChatFriendRemoveDialog
          key={removeTarget.peerId}
          friend={removeTarget}
          onOpenChange={(open) => {
            if (!open) setRemoveTarget(null);
          }}
        />
      ) : null}
    </>
  );
}
