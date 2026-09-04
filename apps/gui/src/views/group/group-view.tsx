import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { MessagesSquare, UsersRound } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
import { Button } from "@/components/ui/button";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { GroupCreateDialog } from "./group-create-dialog";
import { GroupConversation } from "./group-conversation";
import { GroupList } from "./group-list";
import { GroupMemberPanel } from "./group-member-panel";

// 群聊页（G3 完整视图）：左群列表（建群入口 + state 置底）右会话区
// （消息渲染/输入条），成员管理面板由会话头「管理」进入。
// 选群经 URL ?g= 同步，供 1:1 会话列表混排跳转直达。
export function GroupView() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const groups = useGroupStore((s) => s.groups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const groupsError = useGroupStore((s) => s.groupsError);
  const selectedGroupId = useGroupStore((s) => s.selectedGroupId);
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const refreshSelf = useGroupStore((s) => s.refreshSelf);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const subscribeEvents = useGroupStore((s) => s.subscribeEvents);
  const [createOpen, setCreateOpen] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);

  useEffect(() => {
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeEvents();
  }, [loadGroups, refreshSelf, ensureFriends, subscribeEvents]);

  // URL ?g= 预选（外部跳转入口）；selectGroup 自带已加载短路。
  const requested = searchParams.get("g");
  useEffect(() => {
    if (requested) void selectGroup(requested);
  }, [requested, selectGroup]);

  const handleSelect = useCallback(
    (groupId: string) => {
      void selectGroup(groupId);
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.set("g", groupId);
          return next;
        },
        { replace: true },
      );
    },
    [selectGroup, setSearchParams],
  );

  const reload = useCallback(() => {
    void loadGroups();
    void refreshSelf();
  }, [loadGroups, refreshSelf]);

  const selectedGroup =
    groups.find((g) => g.groupId === selectedGroupId) ?? null;

  return (
    <>
      <PageHeader titleKey="group.title" descriptionKey="group.description" />
      <div
        data-testid="group-grid"
        className="grid min-h-0 flex-1 grid-cols-[16rem_1fr] gap-4"
      >
        <section
          aria-label={t("group.groups")}
          className="flex min-h-0 flex-col rounded-lg border"
        >
          <div className="flex items-center justify-between gap-2 px-3 py-2">
            <h2 className="font-medium">{t("group.groups")}</h2>
            <Button
              type="button"
              variant="outline"
              size="sm"
              data-testid="group-create"
              onClick={() => setCreateOpen(true)}
            >
              <UsersRound aria-hidden />
              {t("group.create.action")}
            </Button>
          </div>
          <div
            data-testid="group-scroll"
            className="scroll-slim flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
            <GroupList
              groups={groups}
              loading={!groupsLoaded && !groupsError}
              error={groupsError}
              selectedGroupId={selectedGroupId}
              onSelect={handleSelect}
              onReload={reload}
            />
          </div>
        </section>
        <section
          aria-label={t("group.conversation")}
          className="flex min-h-0 flex-col rounded-lg border"
        >
          {selectedGroup ? (
            <GroupConversation
              key={selectedGroup.groupId}
              group={selectedGroup}
              onOpenManage={() => setManageOpen(true)}
            />
          ) : (
            <EmptyState
              className="max-w-none flex-1"
              icon={MessagesSquare}
              title={t("group.conversationEmpty")}
              description={t("group.conversationEmptyHint")}
            />
          )}
        </section>
      </div>
      <GroupCreateDialog open={createOpen} onOpenChange={setCreateOpen} />
      {selectedGroup && manageOpen ? (
        <GroupMemberPanel
          group={selectedGroup}
          open
          onOpenChange={setManageOpen}
        />
      ) : null}
    </>
  );
}
