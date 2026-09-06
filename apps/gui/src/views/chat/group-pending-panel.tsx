import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { UsersRound } from "lucide-react";

import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";

// 同意入群后的跳转时序兜底（IMC3 需求 1）：roster 事件未到、群尚不在列表时
// 呈加载态不白屏；roster 到达（chat_group_state → group-store upsert）自动
// 进入会话。超时上限兜底异常路径，给出可读未找到态而非无限转圈。
const GROUP_WAIT_TIMEOUT_MS = 10_000;

export function GroupPendingPanel({ groupId }: { groupId: string }) {
  const { t } = useTranslation();
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const [timedOut, setTimedOut] = useState(false);

  useEffect(() => {
    // groupId 变化经调用方 key 重挂载复位；列表从未加载过则主动补拉一次
    if (!groupsLoaded) void loadGroups();
    const timer = window.setTimeout(() => setTimedOut(true), GROUP_WAIT_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, [groupsLoaded, loadGroups]);

  if (timedOut) {
    return (
      <div data-testid="group-pending-timeout" className="flex flex-1 items-center justify-center">
        <EmptyState
          icon={UsersRound}
          title={t("chat.groupPending.timeout")}
          description={groupId}
        />
      </div>
    );
  }
  return (
    <div
      data-testid="group-pending"
      className="text-muted-foreground flex flex-1 items-center justify-center text-sm"
      role="status"
    >
      {t("chat.groupPending.loading")}
    </div>
  );
}
