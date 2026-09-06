import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { useChatStore } from "@/stores/chat-store";

import { FriendInviteSection } from "./friend-invite-section";
import { GroupInviteSection } from "./group-invite-section";

// 消息中心（IMC3 需求 2，/messages）：入群邀请与好友邀请两组列表统一处理。
// 入口：顶栏铃铛 + rail 常驻项（F15，徽标 = 两类 in 向 pending 之和，同源
// selector）。行内操作失败原文上浮；行点击跳对应会话（群 or 好友聊天）。
export function MessagesPage() {
  const { t } = useTranslation();
  const loadInvites = useChatStore((s) => s.loadInvites);
  const loadGroupInvites = useChatStore((s) => s.loadGroupInvites);

  useEffect(() => {
    void loadInvites();
    void loadGroupInvites();
  }, [loadInvites, loadGroupInvites]);

  return (
    <section data-testid="messages-page" className="flex min-h-0 flex-1 flex-col gap-4">
      <header className="flex flex-col gap-1">
        <h1 className="text-lg font-semibold tracking-tight">{t("messages.title")}</h1>
        <p className="text-muted-foreground text-sm">{t("messages.description")}</p>
      </header>
      <div className="scroll-slim flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pb-4">
        <GroupInviteSection />
        <FriendInviteSection />
      </div>
    </section>
  );
}
