import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router-dom";

import { PageHeader } from "@/components/page/page-header";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";

import { AnchorBar } from "./anchor-bar";
import {
  isContactsSectionId,
  type ContactsSectionId,
} from "./contacts-sections";
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

  useEffect(() => {
    void loadFriends();
    void loadInvites();
    void subscribeChatEvents();
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeGroupEvents();
  }, [loadFriends, loadInvites, subscribeChatEvents, loadGroups, refreshSelf, ensureFriends, subscribeGroupEvents]);

  // /contacts#friends 等 hash 深链（5.2 命令面板通讯录锚点）：hash 变化
  // 即定位。hash → 高亮为渲染期状态调整（勿放 effect，react-hooks 纪律）；
  // DOM 滚动属外部系统同步，留 effect。
  const initialHash = location.hash.replace(/^#/, "");
  const [lastHash, setLastHash] = useState(location.hash);
  const [active, setActive] = useState<ContactsSectionId>(() =>
    isContactsSectionId(initialHash) ? initialHash : "friends",
  );
  if (lastHash !== location.hash) {
    setLastHash(location.hash);
    const next = location.hash.replace(/^#/, "");
    if (isContactsSectionId(next)) setActive(next);
  }

  const scrollTo = useCallback((id: ContactsSectionId) => {
    setActive(id);
    document.getElementById(id)?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, []);

  useEffect(() => {
    const hash = location.hash.replace(/^#/, "");
    if (isContactsSectionId(hash)) {
      document.getElementById(hash)?.scrollIntoView({ block: "start", behavior: "smooth" });
    }
  }, [location.hash]);

  // 滚动监听高亮当前节：以视口上沿附近最近分节为准（订阅回调，非 effect 体）
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
