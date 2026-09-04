import { UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { GroupJson } from "@/lib/ipc-types";
import { EmptyState } from "@/views/shared/empty-state";

interface GroupListProps {
  groups: GroupJson[];
  loading: boolean;
  error: string | null;
  onReload: () => void;
}

// 群状态徽标（契约 state 四态）：非 active 置灰/标红，历史保留可辨。
const STATE_VARIANT: Record<
  GroupJson["state"],
  "default" | "secondary" | "destructive" | "outline"
> = {
  active: "default",
  left: "secondary",
  kicked: "destructive",
  disbanded: "outline",
};

function GroupRow({ group }: { group: GroupJson }) {
  const { t } = useTranslation();
  return (
    <div
      data-testid="group-row"
      className="hover:bg-sidebar-accent flex flex-col gap-1 rounded-md px-3 py-2"
    >
      <div className="flex items-center gap-2">
        <span className="truncate text-sm font-medium">{group.name}</span>
        <Badge variant={STATE_VARIANT[group.state]}>
          {t(`group.state.${group.state}`)}
        </Badge>
      </div>
      <div className="text-muted-foreground flex items-center gap-2 text-xs">
        <span className="font-medium">
          {t("group.ownerLabel")} {group.owner.slice(0, 8)}
        </span>
        <span>{t("group.members", { count: group.members.length })}</span>
      </div>
    </div>
  );
}

// 群列表（G2 空页壳）：全量渲染含 left/kicked/disbanded（state 徽标可辨）；
// 建群/管理面板与消息视图由 G3 接入。
export function GroupList({ groups, loading, error, onReload }: GroupListProps) {
  const { t } = useTranslation();

  if (loading) {
    return <p className="px-3 py-2 text-sm">{t("group.loading")}</p>;
  }
  if (error) {
    return (
      <div className="flex flex-col gap-2 px-3 py-2">
        <p className="text-destructive text-sm">{error}</p>
        <Button variant="outline" size="sm" onClick={onReload}>
          {t("common.actions.refresh")}
        </Button>
      </div>
    );
  }
  if (groups.length === 0) {
    return (
      <EmptyState
        icon={UsersRound}
        title={t("group.groupsEmpty")}
        description={t("group.groupsEmptyHint")}
      />
    );
  }
  return (
    <div data-testid="group-list" className="flex flex-col gap-0.5 p-1.5">
      {groups.map((group) => (
        <GroupRow key={group.groupId} group={group} />
      ))}
    </div>
  );
}
