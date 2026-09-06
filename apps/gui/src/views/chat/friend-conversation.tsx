import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Share2 } from "lucide-react";

import { Composer } from "@/components/chat/composer";
import { MessageList } from "@/components/chat/message-list";
import { NodeStoppedCard } from "@/components/chat/node-stopped-card";
import { PeerStatusDot } from "@/components/chat/peer-status";
import { useRetrySend } from "@/components/chat/use-retry-send";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { useNodeStore, usePeerOnline } from "@/stores/node-store";
import { ShareCreateDialog } from "@/acp/components/share-create-dialog";
import { toastSuccess } from "@/components/feedback/toast";

// 1:1 会话记录区（§2.1 右栏 friend 形态）：复用 message-list/composer，
// 不重写消息渲染。key=peer 挂载（引用预览随会话切换自动复位）。
// 会话工具条「分享 ACP」（acp-share §8）：生成链接后作为普通文本消息发出，
// 链接即正文，不改 IM 线协议。
export function FriendConversation({ peer }: { peer: string }) {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const messagesByPeer = useChatStore((s) => s.messagesByPeer);
  const historyLoadingAll = useChatStore((s) => s.historyLoading);
  const hasMoreAll = useChatStore((s) => s.hasMore);
  const loadOlder = useChatStore((s) => s.loadOlder);
  const cancelPending = useChatStore((s) => s.cancelPending);
  const sendText = useChatStore((s) => s.sendText);
  const messages = messagesByPeer[peer] ?? [];
  const historyLoading = historyLoadingAll[peer] ?? false;
  const hasMore = hasMoreAll[peer] ?? false;
  const selectedFriend = friends.find((f) => f.peerId === peer);
  const online = usePeerOnline(peer);
  // 节点未运行判定（IM-T51）：仅 status 已加载且 running=false 才引导
  const nodeStatus = useNodeStore((s) => s.status);
  const nodeStopped = nodeStatus !== null && !nodeStatus.running;
  const retrySend = useRetrySend(peer);
  const [replyTarget, setReplyTarget] = useState<ChatMessageJson | null>(null);
  const [shareOpen, setShareOpen] = useState(false);

  const sendShareLink = async (link: string) => {
    await sendText(peer, link);
    toastSuccess(t("acp.share.sent"));
  };

  return (
    <>
      <div data-testid="chat-conversation-header" className="shrink-0 border-b px-4 py-2">
        <div className="flex items-center gap-2 text-sm font-medium">
          <span>{selectedFriend?.nickname || peer.slice(0, 8)}</span>
          <PeerStatusDot online={online} testId="chat-header-status" withLabel />
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground ml-auto inline-flex items-center gap-1 text-xs"
            onClick={() => setShareOpen(true)}
            data-testid="chat-share-acp"
          >
            <Share2 aria-hidden className="size-3.5" />
            {t("acp.share.dialogTitle")}
          </button>
        </div>
        <div className="text-muted-foreground text-xs">{peer}</div>
      </div>
      <MessageList
        peer={peer}
        messages={messages}
        loadingOlder={historyLoading}
        hasMore={hasMore}
        onLoadOlder={() => void loadOlder(peer)}
        onCancelPending={(id) => cancelPending(peer, id)}
        onReply={setReplyTarget}
        onRetry={(message) => void retrySend(message)}
      />
      {nodeStopped ? <NodeStoppedCard /> : null}
      <Composer
        peer={peer}
        replyTarget={replyTarget}
        onReplyCancel={() => setReplyTarget(null)}
        disabled={nodeStopped}
      />
      <ShareCreateDialog
        open={shareOpen}
        onOpenChange={setShareOpen}
        onSendLink={sendShareLink}
      />
    </>
  );
}
