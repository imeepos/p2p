import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { SegmentedControl } from "@/components/ui/segmented-control";
import { useChatStore } from "@/stores/chat-store";

import { FriendInviteSection } from "./friend-invite-section";
import { GroupInviteSection } from "./group-invite-section";
import type { MessagesView } from "./section-header";

// 消息中心（IMC3 需求 2，/messages）：入群邀请与好友邀请两组列表统一处理。
// 入口：顶栏铃铛 + rail 常驻项（F15，徽标 = 两类 in 向 pending 之和，同源
// selector）。行内操作失败原文上浮；行点击跳对应会话（群 or 好友聊天）。
// 待处理/历史双视图（设计稿增量 3）：默认只显示待处理；历史视图含群邀请
// 终态与好友历史空态（收件箱即待处理集，契约无 state 字段）。
export function MessagesPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<MessagesView>("pending");
  const loadInvites = useChatStore((s) => s.loadInvites);
  const loadGroupInvites = useChatStore((s) => s.loadGroupInvites);
  const groupInvites = useChatStore((s) => s.groupInvites);
  const invites = useChatStore((s) => s.invites);

  useEffect(() => {
    void loadInvites();
    void loadGroupInvites();
  }, [loadInvites, loadGroupInvites]);

  // 分段控件计数 = 待处理视图将显示的行数（群 pending 两向 + 好友收件箱
  // 全集），与列表同源；rail 铃铛角标口径不同（仅 in 向，F15），不混用。
  const pendingCount =
    groupInvites.filter((invite) => invite.state === "pending").length +
    invites.length;

  return (
    <section data-testid="messages-page" className="flex min-h-0 flex-1 flex-col gap-4">
      <header className="flex flex-wrap items-start justify-between gap-2">
        <div className="flex flex-col gap-1">
          <h1 className="text-lg font-semibold tracking-tight">{t("messages.title")}</h1>
          <p className="text-muted-foreground text-sm">{t("messages.description")}</p>
        </div>
        <SegmentedControl
          value={view}
          onChange={setView}
          ariaLabel={t("messages.title")}
          options={[
            { value: "pending", label: t("messages.view.pending"), count: pendingCount },
            { value: "history", label: t("messages.view.history") },
          ]}
        />
      </header>
      <div className="scroll-slim flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pb-4">
        <GroupInviteSection view={view} />
        <FriendInviteSection view={view} />
      </div>
    </section>
  );
}
