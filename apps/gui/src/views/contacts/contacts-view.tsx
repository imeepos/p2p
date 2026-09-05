import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router-dom";

import { PageHeader } from "@/components/page/page-header";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";

import { AnchorBar, isContactsSectionId, type ContactsSectionId } from "./anchor-bar";
import { AgentSection } from "./agent-section";
import { FriendSection } from "./friend-section";
import { GroupSection } from "./group-section";
import { InviteInbox } from "./invite-inbox";

// 通讯录页（app-shell-redesign §3）：纵向三分节 + 页顶锚点条（点击滚动
// 定位、当前节高亮、/contacts#* 深链定位）；页顶待处理邀请收件箱（§3.2）。
export function ContactsView() {
  const location = useLocation();
  const loadFriends = useChatStore((s) => s.loadFriends);
  const loadInvites = useChatStore((s) => s.loadInvites);
  const subscribeChatEvents = useChatStore((s) => s.subscribeEvents);
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const refreshSelf = useGroupStore((s) => s.refreshSelf);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const subscribeGroupEvents = useGroupStore((s) => s.subscribeEvents);
  const [active, setActive] = useState<ContactsSectionId>("friends");

  useEffect(() => {
    void loadFriends();
    void loadInvites();
    void subscribeChatEvents();
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeGroupEvents();
  }, [loadFriends, loadInvites, subscribeChatEvents, loadGroups, refreshSelf, ensureFriends, subscribeGroupEvents]);

  const scrollTo = useCallback((id: ContactsSectionId) => {
    setActive(id);
    document.getElementById(id)?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, []);

  // /contacts#friends 等 hash 深链（5.2 命令面板通讯录锚点）：落定即定位
  useEffect(() => {
    const hash = location.hash.replace(/^#/, "");
    if (isContactsSectionId(hash)) scrollTo(hash);
  }, [location.hash, scrollTo]);

  // 滚动监听高亮当前节：以视口上沿附近最近分节为准
  useEffect(() => {
    const onScroll = () => {
      let current: ContactsSectionId = "friends";
      for (const id of ["groups", "agents"] as const) {
        const el = document.getElementById(id);
        if (el && el.getBoundingClientRect().top <= 120) current = id;
      }
      setActive((prev) => (prev === current ? prev : current));
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <ConfirmProvider>
      <div className="col-span-12 flex flex-col gap-4">
        <PageHeader titleKey="contacts.title" descriptionKey="contacts.description" />
        <InviteInbox />
        <AnchorBar active={active} onGo={scrollTo} />
        <FriendSection />
        <GroupSection />
        <AgentSection />
      </div>
    </ConfirmProvider>
  );
}
