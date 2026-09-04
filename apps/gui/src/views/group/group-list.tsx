import { UsersRound } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { GroupChatState, GroupJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";
import { orderedGroups } from "./group-names";

interface GroupListProps {
  groups: GroupJson[];
  loading: boolean;
  error: string | null;
  selectedGroupId: string | null;
  onSelect: (groupId: string) => void;
  onReload: () => void;
}

// 群状态徽标（契约 state 四态）：非 active 置灰/标红，历史保留可辨。
const STATE_VARIANT: Record<GroupChatState, "default" | "secondary" | "destructive" | "outline"> = {
  active: "default",
  left: "secondary",
  kicked: "destructive",
  disbanded: "outline",
};

// 状态徽标：群列表行与会话头共用（G3）。
export function GroupStateBadge({ state }: { state: GroupChatState }) {
  const { t } = useTranslation();
  return (
    <Badge variant={STATE_VARIANT[state]} data-testid={`group-state-${state}`}>
      {t(`group.state.${state}`)}
    </Badge>
  );
}

function GroupRow({
  group,
  isActive,
  onSelect,
}: {
  group: GroupJson;
  isActive: boolean;
  onSelect: (groupId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="group-row"
      onClick={() => onSelect(group.groupId)}
      className={cn(
        "hover:bg-sidebar-accent flex w-full flex-col gap-1 rounded-md px-3 py-2 text-left",
        isActive && "bg-accent",
      )}
    >
      <div className="flex items-center gap-2">
        <span className="truncate text-sm font-medium">{group.name}</span>
        {group.state !== "active" ? <GroupStateBadge state={group.state} /> : null}
      </div>
      <div className="text-muted-foreground flex items-center gap-2 text-xs">
        <span className="font-medium">
          {t("group.ownerLabel")} {group.owner.slice(0, 8)}
        </span>
        <span>{t("group.members", { count: group.members.length })}</span>
      </div>
    </button>
  );
}

// 群列表：全量渲染含非 active（置底 + 徽标可辨），行点击选会话。
export function GroupList({
  groups,
  loading,
  error,
  selectedGroupId,
  onSelect,
  onReload,
}: GroupListProps) {
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
      {orderedGroups(groups).map((group) => (
        <GroupRow
          key={group.groupId}
          group={group}
          isActive={group.groupId === selectedGroupId}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
