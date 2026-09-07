import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Share2 } from "lucide-react";

import { Composer } from "@/components/chat/composer";
import { MessageList } from "@/components/chat/message-list";
import type { BubbleAvatar } from "@/components/chat/message-bubble";
import { NodeStoppedCard } from "@/components/chat/node-stopped-card";
import { PeerStatusDot } from "@/components/chat/peer-status";
import { useRetrySend } from "@/components/chat/use-retry-send";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { useNodeStore, usePeerOnline } from "@/stores/node-store";
import { useProfileStore } from "@/stores/profile-store";
import { ShareCreateDialog } from "@/acp/components/share-create-dialog";
import { toastSuccess } from "@/components/feedback/toast";
import { isFailedSendReport, notifyFailedSendReport } from "@/components/chat/send-notify";

// WX1 微信风格会话区：居中标题 + 右侧动作；气泡带外侧头像（本机走资料头像）。
// key=peer 挂载（引用预览随会话切换自动复位）。
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
  const profile = useProfileStore((s) => s.profile);
  const [replyTarget, setReplyTarget] = useState<ChatMessageJson | null>(null);
  const [shareOpen, setShareOpen] = useState(false);

  const peerName = selectedFriend?.nickname || peer.slice(0, 8);
  const peerAvatar: BubbleAvatar = { label: peerName, seed: peer };
  const selfAvatar: BubbleAvatar = {
    label: profile.name.trim() || "P2P",
    seed: "self",
    src: profile.avatar,
  };

  const sendShareLink = async (link: string) => {
    const report = await sendText(peer, link);
    // W1-02：mark_failed 不抛错，假成功 toast 会误导——失败走失败信号
    if (isFailedSendReport(report)) {
      notifyFailedSendReport(report);
      return;
    }
    toastSuccess(t("acp.share.sent"));
  };

  return (
    <>
      <div
        data-testid="chat-conversation-header"
        className="relative flex h-12 shrink-0 items-center border-b border-border/60 px-4"
      >
        <div className="absolute left-1/2 flex -translate-x-1/2 items-center gap-2 text-sm font-medium">
          <span title={peer}>{peerName}</span>
          <PeerStatusDot online={online} testId="chat-header-status" withLabel />
        </div>
        <button
          type="button"
          className="text-muted-foreground hover:text-foreground ml-auto inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs hover:bg-wx-hover"
          onClick={() => setShareOpen(true)}
          data-testid="chat-share-acp"
        >
          <Share2 aria-hidden className="size-3.5" />
          {t("acp.share.dialogTitle")}
        </button>
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
        selfAvatar={selfAvatar}
        peerAvatar={peerAvatar}
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
