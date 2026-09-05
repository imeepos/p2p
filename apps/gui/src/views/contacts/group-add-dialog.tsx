import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { InboxIcon, UsersRoundIcon } from "lucide-react";

interface GroupAddDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: () => void;
}

type Mode = "choose" | "invites";

// 群区「添加」二选一弹框（§3.2 入群/邀请）：创建群（成为 owner）/ 处理
// 收到的入群邀请。当前契约（gui-contract group_invite：owner 直接 rev+1
// 写入 members 并推 roster，无待确认流，im-group-design v1 明确不做邀
// 请确认流），受邀侧如实呈现已入群清单与说明，不自造 pending 状态机。
export function GroupAddDialog({ open, onOpenChange, onCreate }: GroupAddDialogProps) {
  const { t } = useTranslation();
  const groups = useGroupStore((s) => s.groups);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const [mode, setMode] = useState<Mode>("choose");

  const joinedByInvite = groups.filter(
    (g: GroupJson) => g.state === "active" && g.owner !== selfPeerId,
  );

  const close = (next: boolean) => {
    if (!next) setMode("choose");
    onOpenChange(next);
  };

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-md" data-testid="contacts-group-add-dialog">
        <DialogHeader>
          <DialogTitle>{t("contacts.groupAdd.title")}</DialogTitle>
          <DialogDescription>{t("group.description")}</DialogDescription>
        </DialogHeader>
        {mode === "choose" ? (
          <div className="flex flex-col gap-2">
            <button
              type="button"
              className="hover:bg-accent flex items-center gap-3 rounded-md border p-3 text-left"
              data-testid="contacts-group-add-create"
              onClick={() => {
                close(false);
                onCreate();
              }}
            >
              <UsersRoundIcon aria-hidden className="size-5" />
              <span>
                <span className="block text-sm font-medium">{t("contacts.groupAdd.createTab")}</span>
                <span className="text-muted-foreground block text-xs">{t("group.create.namePlaceholder")}</span>
              </span>
            </button>
            <button
              type="button"
              className="hover:bg-accent flex items-center gap-3 rounded-md border p-3 text-left"
              data-testid="contacts-group-add-invites"
              onClick={() => setMode("invites")}
            >
              <InboxIcon aria-hidden className="size-5" />
              <span>
                <span className="block text-sm font-medium">{t("contacts.groupAdd.invitesTab")}</span>
                <span className="text-muted-foreground block text-xs">{t("contacts.groupAdd.invitesHint")}</span>
              </span>
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-2" data-testid="contacts-group-invites-list">
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
            <p className="text-muted-foreground text-xs">{t("contacts.groupAdd.invitesHint")}</p>
            <Button type="button" variant="outline" size="sm" className="w-fit" onClick={() => setMode("choose")}>
              {t("common.actions.back")}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
