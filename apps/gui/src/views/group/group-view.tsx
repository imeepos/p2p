import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { MessagesSquare } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
import { ipc } from "@/lib/ipc";
import type { GroupJson } from "@/lib/ipc-types";
import { EmptyState } from "@/views/shared/empty-state";
import { GroupList } from "./group-list";

// 群聊页（G2 空页壳）：路由/菜单已注册，群列表走 ipc.groupList（mock 同签名
// 可交互演示）；会话区为空态占位，建群/成员面板/消息渲染由 G3 接入。
export function GroupView() {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<GroupJson[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // load 只做异步取数（.then/.catch 内 setState），供 effect 与刷新按钮直接调用；
  // 失败落 error 状态与 console，不静默。
  const load = useCallback(() => {
    ipc.groupList()
      .then((list) => {
        setGroups(list);
        setError(null);
        setLoaded(true);
      })
      .catch((err) => {
        console.error("[group] 群列表加载失败", err);
        setError(err instanceof Error ? err.message : String(err));
        setLoaded(true);
      });
  }, []);

  useEffect(() => {
    load();
  }, [load]);

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
          </div>
          <div
            data-testid="group-scroll"
            className="scroll-slim flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
            <GroupList
              groups={groups}
              loading={!loaded && !error}
              error={error}
              onReload={load}
            />
          </div>
        </section>
        <section
          aria-label={t("group.conversation")}
          className="flex min-h-0 flex-col rounded-lg border"
        >
          <EmptyState
            className="max-w-none flex-1"
            icon={MessagesSquare}
            title={t("group.conversationEmpty")}
            description={t("group.conversationEmptyHint")}
          />
        </section>
      </div>
    </>
  );
}
