import { useTranslation } from "react-i18next";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { GroupCreateForm } from "@/views/group/group-create-form";
import { InboxIcon } from "lucide-react";

interface GroupAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 群区「添加」弹框（F09 重构）：默认页签直接呈现建群表单（省掉二选一中转
// 一跳），「收到的入群邀请」降为次级页签。当前契约（gui-contract
// group_invite：owner 直接 rev+1 写入 members 并推 roster，无待确认流，
// im-group-design v1 明确不做邀请确认流），受邀侧如实呈现已入群清单与说明，
// 不自造 pending 状态机。
export function GroupAddDialog({ open, onOpenChange }: GroupAddDialogProps) {
  const { t } = useTranslation();
  const groups = useGroupStore((s) => s.groups);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);

  const joinedByInvite = groups.filter(
    (g: GroupJson) => g.state === "active" && g.owner !== selfPeerId,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" data-testid="contacts-group-add-dialog">
        <DialogHeader>
          <DialogTitle>{t("contacts.groupAdd.title")}</DialogTitle>
          <DialogDescription>{t("group.description")}</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="create">
          <TabsList>
            <TabsTrigger value="create" data-testid="contacts-group-add-create">
              {t("contacts.groupAdd.createTab")}
            </TabsTrigger>
            <TabsTrigger value="invites" data-testid="contacts-group-add-invites">
              {t("contacts.groupAdd.invitesTab")}
            </TabsTrigger>
          </TabsList>
          <TabsContent value="create">
            <GroupCreateForm onDone={() => onOpenChange(false)} />
          </TabsContent>
          <TabsContent value="invites" data-testid="contacts-group-invites-list">
            {joinedByInvite.length === 0 ? (
              <EmptyState icon={InboxIcon} title={t("contacts.groupAdd.invitesEmpty")} />
            ) : (
              joinedByInvite.map((group) => (
                <div
                  key={group.groupId}
                  className="flex items-center gap-2 rounded-md border px-2 py-1.5"
                  data-testid={"contacts-group-invited-" + group.groupId}
                >
                  <span className="truncate text-sm">{group.name}</span>
                  <span className="text-muted-foreground ml-auto text-xs">
                    {t("group.state.active")}
                  </span>
                </div>
              ))
            )}
            <p className="text-muted-foreground mt-2 text-xs">
              {t("contacts.groupAdd.invitesHint")}
            </p>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
