import { useCallback, useEffect, useState } from "react";
import { useLocation, useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { SearchIcon, UserRoundPlusIcon } from "lucide-react";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useAcpStore } from "@/acp/acp-store";
import { useAuthzStore } from "@/stores/authz-store";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";

import {
  ContactsPaneContext,
  isContactsSectionId,
  loadCollapsedSections,
  saveCollapsedSections,
  type ContactsPaneApi,
  type ContactsSectionId,
} from "./contacts-sections";
import {
  firstAvailableEntity,
  resolveEntity,
  selectionKey,
  type ContactsData,
  type ContactSelection,
} from "./contacts-detail-model";
import { ContactsDetailPane } from "./contacts-detail";
import { AgentSection } from "./agent-section";
import { FriendSection } from "./friend-section";
import { GroupSection } from "./group-section";
import { InviteInbox } from "./invite-inbox";

// 通讯录页（微信通讯录式双栏改版）：左栏树（检索 + 添加好友 + 新的
// 朋友/好友/群聊/Agent 折叠分节）+ 右栏选中实体资料卡；/contacts#*
// 深链与分节锚点契约保持不变。
export function ContactsView() {
  const { t } = useTranslation();
  const location = useLocation();
  const [searchParams, setSearchParams] = useSearchParams();
  const loadFriends = useChatStore((s) => s.loadFriends);
  const loadInvites = useChatStore((s) => s.loadInvites);
  const subscribeChatEvents = useChatStore((s) => s.subscribeEvents);
  const friends = useChatStore((s) => s.friends);
  const invites = useChatStore((s) => s.invites) ?? [];
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const refreshSelf = useGroupStore((s) => s.refreshSelf);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const subscribeGroupEvents = useGroupStore((s) => s.subscribeEvents);
  const groups = useGroupStore((s) => s.groups);
  const agents = useAcpStore((s) => s.saved);
  const loadAuthz = useAuthzStore((s) => s.loadAll);

  useEffect(() => {
    void loadFriends();
    void loadInvites();
    void subscribeChatEvents();
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeGroupEvents();
    void loadAuthz();
  }, [loadFriends, loadInvites, subscribeChatEvents, loadGroups, refreshSelf, ensureFriends, subscribeGroupEvents, loadAuthz]);

  // /contacts#friends 等 hash 深链（5.2 命令面板通讯录锚点）：hash 变化
  // 即定位。hash → 高亮为渲染期状态调整（勿放 effect，react-hooks 纪律）；
  // DOM 滚动属外部系统同步，留 effect。
  const initialHash = location.hash.replace(/^#/, "");
  const [lastHash, setLastHash] = useState(location.hash);
  const [active, setActive] = useState<ContactsSectionId | null>(() =>
    isContactsSectionId(initialHash) ? initialHash : null,
  );
  if (lastHash !== location.hash) {
    setLastHash(location.hash);
    const next = location.hash.replace(/^#/, "");
    if (isContactsSectionId(next)) setActive(next);
  }

  useEffect(() => {
    const hash = location.hash.replace(/^#/, "");
    if (isContactsSectionId(hash)) {
      document.getElementById(hash)?.scrollIntoView({ block: "start", behavior: "smooth" });
    }
  }, [location.hash]);

  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ContactSelection | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => loadCollapsedSections());

  const toggleSection = useCallback((id: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      saveCollapsedSections(next);
      return next;
    });
  }, []);

  // 锚点定位：置高亮 + 展开目标节（折叠态下锚点仍可达）+ 滚动到节首。
  const gotoSection = useCallback(
    (id: ContactsSectionId) => {
      setActive(id);
      setCollapsed((prev) => {
        if (!prev.has(id)) return prev;
        const next = new Set(prev);
        next.delete(id);
        saveCollapsedSections(next);
        return next;
      });
      document.getElementById(id)?.scrollIntoView({ block: "start", behavior: "smooth" });
    },
    [],
  );

  const data: ContactsData = { friends, invites, groups, agents };
  const resolved = selected ? resolveEntity(selected, data) : null;
  const effective = resolved ?? firstAvailableEntity(data);

  // 跨卡 URL 契约：#/contacts?add=<peerId> 开添加好友弹窗并预填（弹窗
  // 关闭清参的既有语义不变）；左栏「添加好友」按钮复用同一参数通道。
  const openFriendAdd = () => {
    const next = new URLSearchParams(searchParams);
    next.set("add", "");
    setSearchParams(next);
  };

  const pane: ContactsPaneApi = {
    query,
    selectedKey: effective ? selectionKey(effective.selection) : null,
    select: setSelected,
    isSectionCollapsed: (id) => collapsed.has(id),
    toggleSection,
    activeSection: active,
    gotoSection,
  };

  return (
    <ConfirmProvider>
      <ContactsPaneContext.Provider value={pane}>
        <div className="flex min-h-0 flex-1 gap-4">
          <aside
            className="bg-card ring-border flex w-72 shrink-0 flex-col overflow-hidden rounded-lg ring-1"
            aria-label={t("contacts.title")}
          >
            <div className="border-border flex flex-col gap-2 border-b p-3">
              <label className="relative block">
                <SearchIcon
                  aria-hidden
                  className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
                />
                <Input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t("contacts.tree.search")}
                  aria-label={t("contacts.tree.search")}
                  className="h-9 pl-8"
                  autoComplete="off"
                  data-testid="contacts-search-global"
                />
              </label>
              <Button
                type="button"
                variant="outline"
                className="w-full justify-center"
                onClick={openFriendAdd}
                data-testid="contacts-friend-add"
              >
                <UserRoundPlusIcon aria-hidden className="size-4" />
                {t("contacts.friends.add")}
              </Button>
            </div>
            <nav
              aria-label={t("contacts.title")}
              data-testid="contacts-anchor-bar"
              className="min-h-0 flex-1 overflow-y-auto p-2"
            >
              <InviteInbox />
              <FriendSection />
              <GroupSection />
              <AgentSection />
            </nav>
          </aside>
          <section
            aria-label={t("contacts.title")}
            data-testid="contacts-detail-pane"
            className="bg-card ring-border min-w-0 flex-1 overflow-y-auto rounded-lg ring-1"
          >
            <ContactsDetailPane entity={effective} />
          </section>
        </div>
      </ContactsPaneContext.Provider>
    </ConfirmProvider>
  );
}
