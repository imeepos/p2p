import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { MessageSquare, UserPlusIcon } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
import { Button } from "@/components/ui/button";
import { ChatFriendAddDialog } from "@/components/chat/chat-friend-add-dialog";
import { ChatFriendMoveDialog } from "@/components/chat/chat-friend-move-dialog";
import { ChatFriendRemoveDialog } from "@/components/chat/chat-friend-remove-dialog";
import { ConversationList } from "@/components/chat/conversation-list";
import { PeerStatusDot } from "@/components/chat/peer-status";
import type { ChatFriendJson, ChatMessageJson } from "@/lib/ipc-types";
import { Composer } from "@/components/chat/composer";
import { MessageList } from "@/components/chat/message-list";
import { NodeStoppedCard } from "@/components/chat/node-stopped-card";
import { useRetrySend } from "@/components/chat/use-retry-send";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useNodeStore, usePeerOnline } from "@/stores/node-store";
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
  // 群会话混排（G3）：群列表来自 group-store，点击行跳群聊页并带 ?g= 预选
  const groups = useGroupStore((s) => s.groups);
  const selectedGroupId = useGroupStore((s) => s.selectedGroupId);
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const subscribeGroupEvents = useGroupStore((s) => s.subscribeEvents);
  const [addFriendOpen, setAddFriendOpen] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ChatFriendJson | null>(null);
  // 移动分组目标（IM-T43）：行内「移动到分组」入口，对话框承载输入与 IPC 调用
  const [moveTarget, setMoveTarget] = useState<ChatFriendJson | null>(null);
  // 回复引用预览（IM-T46B）：页内状态即可（Composer/MessageList 同树）；
  // 清空走 onSelect 事件路径与发送成功回调，避免把 A 会话的引用带进 B 会话发送。
  const [replyTarget, setReplyTarget] = useState<ChatMessageJson | null>(null);

  useEffect(() => {
    void loadFriends();
    void subscribeEvents();
    void loadGroups();
    void subscribeGroupEvents();
  }, [loadFriends, subscribeEvents, loadGroups, subscribeGroupEvents]);

  const selectedFriend = friends.find((f) => f.peerId === selectedPeer);
  const selectedOnline = usePeerOnline(selectedPeer ?? "");
  // 节点未运行判定（IM-T51）：仅 status 已加载且 running=false 才引导；
  // status 未加载（null）时保持正常输入，避免启动瞬间误伤。
  const nodeStatus = useNodeStore((s) => s.status);
  const nodeStopped = nodeStatus !== null && !nodeStatus.running;
  const retrySend = useRetrySend(selectedPeer);

  return (
    <>
      <PageHeader titleKey="chat.title" descriptionKey="chat.description" />
      <div
        data-testid="chat-grid"
        className="grid min-h-0 flex-1 grid-cols-[16rem_1fr] gap-4"
      >
        <section
          aria-label={t("chat.friends")}
          className="flex min-h-0 flex-col rounded-lg border"
        >
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
          {/* 列表区内滚域（IM-T52）：卡片头部固定，滚动只发生在本容器 */}
          <div
            data-testid="friends-scroll"
            className="scroll-slim flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
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
              onMoveFriend={(peerId) =>
                setMoveTarget(friends.find((f) => f.peerId === peerId) ?? null)
              }
              onRemoveFriend={(peerId) =>
                setRemoveTarget(friends.find((f) => f.peerId === peerId) ?? null)
              }
              onRetry={async () => {
                await loadFriends();
                const err = useChatStore.getState().friendsError;
                if (err) throw new Error(err);
              }}
              groups={groups}
              selectedGroupId={selectedGroupId}
              onSelectGroup={(groupId) => {
                // App 根为 HashRouter：直接改 hash 即路由跳转；
                // 不经 useNavigate 以免测试树无 Router 上下文时崩溃。
                window.location.hash = `/group?g=${groupId}`;
              }}
            />
          </div>
        </section>

        <section
          aria-label={t("chat.conversation")}
          className="flex min-h-0 flex-col rounded-lg border"
        >
          {selectedPeer ? (
            <>
              <div
                data-testid="chat-conversation-header"
                className="shrink-0 border-b px-4 py-2"
              >
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
                onRetry={(message) => void retrySend(message)}
              />
              {nodeStopped ? <NodeStoppedCard /> : null}
              <Composer
                peer={selectedPeer}
                replyTarget={replyTarget}
                onReplyCancel={() => setReplyTarget(null)}
                disabled={nodeStopped}
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
      {moveTarget ? (
        <ChatFriendMoveDialog
          key={moveTarget.peerId}
          friend={moveTarget}
          onOpenChange={(open) => {
            if (!open) setMoveTarget(null);
          }}
        />
      ) : null}
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
