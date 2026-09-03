import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { ChatPlaceholder } from "@/components/chat/chat-placeholder";
import { PageHeader } from "@/components/page/page-header";
import { ipc } from "@/lib/ipc";
import type { ChatFriendJson } from "@/lib/ipc-types";

export function ChatView() {
  const { t } = useTranslation();
  const [friends, setFriends] = useState<ChatFriendJson[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    ipc.chatFriendsList().then((items) => {
      if (active) setFriends(items);
    }).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[chat] chat_friends_list 失败", error);
      if (active) setLoadError(message);
    });
    return () => { active = false; };
  }, []);

  return (
    <>
      <PageHeader titleKey="chat.title" descriptionKey="chat.description" />
      <div className="grid min-h-72 grid-cols-[16rem_1fr] gap-4">
        <section aria-label={t("chat.friends")} className="rounded-lg border p-3">
          <h2 className="mb-3 font-medium">{t("chat.friends")}</h2>
          {loadError ? <p className="text-sm text-destructive">{loadError}</p> : null}
          {friends.length === 0 && !loadError ? <ChatPlaceholder /> : null}
          {friends.map((friend) => (
            <div key={friend.peerId} className="rounded-md px-2 py-2 text-sm">
              <div className="font-medium">{friend.nickname || friend.peerId.slice(0, 8)}</div>
              <div className="text-muted-foreground">{friend.peerId.slice(0, 12)}</div>
            </div>
          ))}
        </section>
        <section aria-label={t("chat.conversation")} className="rounded-lg border p-4">
          <div className="flex h-full min-h-56 flex-col justify-between">
            <p className="text-muted-foreground">{t("chat.conversationEmpty")}</p>
            <div className="rounded-md border p-3 text-sm text-muted-foreground">
              {t("chat.inputPlaceholder")}
            </div>
          </div>
        </section>
      </div>
    </>
  );
}
