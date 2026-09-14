// 会话侧栏：工作区→会话两级树（uix-spec §2）。组树由 workspace-model 纯函数在
// 组件层派生；开合/搜索等 UI 态在 workspace-ui-store。resume/close/离线禁用等
// 既有行为与 testid 保持不变（P2 确认纪律沿用）。
import { MessagesSquare, Plus, Search } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import {
  COLLAPSED_SESSION_LIMIT,
  collapseGroupRows,
  filterSessionsByQuery,
  groupSessionsByWorkspace,
  isGroupOpen,
  UNGROUPED_KEY,
  type WorkspaceGroupNode,
} from "@/acp/workspace-model";
import { useWorkspaceUiStore } from "@/acp/workspace-ui-store";
import { useAcpStore } from "@/acp/acp-store";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";
import { SessionGroupRow } from "./session-group-row";

function SessionRow(props: {
  sessionId: string;
  title: string;
  active: boolean;
  online: boolean;
  onResume: (sessionId: string) => void;
  onClose: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const handleClose = () => {
    // 破坏性操作确认纪律（P2）：关闭会话先过全站确认弹框，取消则不关
    void confirm({
      title: t("acp.sessions.closeConfirmTitle"),
      description: t("acp.sessions.closeConfirmDescription", { title: props.title }),
      confirmText: t("acp.sessions.closeConfirmAction"),
      cancelText: t("acp.cancel"),
      destructive: true,
    }).then((ok) => {
      if (ok) props.onClose(props.sessionId);
    });
  };
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-3 rounded-lg px-2 py-1",
        "hover:bg-accent",
        props.active && "bg-accent font-medium",
      )}
      data-testid={"acp-session-row-" + props.sessionId}
    >
      <button
        type="button"
        className="min-w-0 flex-1 text-left"
        disabled={!props.online}
        onClick={() => props.onResume(props.sessionId)}
        title={props.sessionId}
        aria-current={props.active ? "true" : undefined}
      >
        <p className="truncate text-sm">{props.title}</p>
        <p className="text-muted-foreground truncate text-xs">{props.sessionId}</p>
      </button>
      <Button
        size="icon"
        variant="ghost"
        className="size-7 shrink-0"
        disabled={!props.online}
        onClick={handleClose}
        aria-label={t("acp.sessions.close")}
        data-testid={"acp-session-close-" + props.sessionId}
      >
        ×
      </Button>
    </div>
  );
}

function GroupSection(props: {
  group: WorkspaceGroupNode;
  groupLabel: string;
  open: boolean;
  transientOpen: boolean;
  activeSessionId: string | null;
  online: boolean;
  onToggle: () => void;
  onExpandHidden: () => void;
  onResume: (sessionId: string) => void;
  onClose: (sessionId: string) => void;
}) {
  const limited = collapseGroupRows(props.group.sessions, { limit: COLLAPSED_SESSION_LIMIT });
  const rows = props.transientOpen ? props.group.sessions : limited.rows;
  const hiddenCount = props.transientOpen ? 0 : limited.hiddenCount;
  return (
    <div className="space-y-0.5">
      <SessionGroupRow
        groupKey={props.group.key}
        label={props.groupLabel}
        open={props.open}
        containsCurrent={props.group.containsCurrent}
        hiddenCount={hiddenCount}
        onToggle={props.onToggle}
        onExpandHidden={props.onExpandHidden}
      />
      {props.open
        ? rows.map((session) => (
            <SessionRow
              key={session.sessionId}
              sessionId={session.sessionId}
              title={session.title ?? session.sessionId}
              active={session.sessionId === props.activeSessionId}
              online={props.online}
              onResume={props.onResume}
              onClose={props.onClose}
            />
          ))
        : null}
    </div>
  );
}

export function SessionSidebar() {
  const { t } = useTranslation();
  const sessions = useAcpStore((s) => s.sessions);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const online = useAcpStore((s) => s.phase) === "online";
  const newSession = useAcpStore((s) => s.newSession);
  const resumeSession = useAcpStore((s) => s.resumeSession);
  const closeSession = useAcpStore((s) => s.closeSession);

  const collapsedKeys = useWorkspaceUiStore((s) => s.collapsedGroupKeys);
  const transientKeys = useWorkspaceUiStore((s) => s.transientExpandedKeys);
  const query = useWorkspaceUiStore((s) => s.workspaceQuery);
  const toggleGroup = useWorkspaceUiStore((s) => s.toggleGroup);
  const expandGroupTransiently = useWorkspaceUiStore((s) => s.expandGroupTransiently);
  const setQuery = useWorkspaceUiStore((s) => s.setWorkspaceQuery);

  // 树在组件层派生（uix-spec §4.2）：store 不冗余存树
  const groups = useMemo(
    () => groupSessionsByWorkspace(sessions, activeSessionId),
    [sessions, activeSessionId],
  );
  const trimmedQuery = query.trim();
  const searchResults = useMemo(
    () => (trimmedQuery ? filterSessionsByQuery(sessions, trimmedQuery) : null),
    [sessions, trimmedQuery],
  );

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">{t("acp.sessions.card")}</CardTitle>
        <Button
          size="sm"
          variant="outline"
          disabled={!online}
          onClick={() => void newSession()}
          data-testid="acp-session-new"
        >
          <Plus className="size-4" aria-hidden />
          {t("acp.sessions.new")}
        </Button>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
        <label className="relative block">
          <Search
            className="text-muted-foreground absolute top-1/2 left-2 size-3.5 -translate-y-1/2"
            aria-hidden
          />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              // Esc 清空收起（uix-spec I8）
              if (event.key === "Escape") setQuery("");
            }}
            placeholder={t("acp.sessions.searchPlaceholder")}
            className={cn(
              "border-input h-8 w-full rounded-lg border bg-transparent pl-7 text-sm",
              "placeholder:text-muted-foreground outline-none focus-visible:border-info",
            )}
            data-testid="acp-session-search"
          />
        </label>
        {sessions.length === 0 ? (
          <EmptyState
            icon={MessagesSquare}
            title={t("acp.sessions.empty")}
            description={t("acp.sessions.emptyHint")}
          />
        ) : searchResults ? (
          <div className="space-y-0.5">
            {searchResults.map((session) => (
              <SessionRow
                key={session.sessionId}
                sessionId={session.sessionId}
                title={session.title ?? session.sessionId}
                active={session.sessionId === activeSessionId}
                online={online}
                onResume={(id) => void resumeSession(id)}
                onClose={(id) => void closeSession(id)}
              />
            ))}
            {searchResults.length === 0 ? (
              <p className="text-muted-foreground px-2 py-3 text-xs">
                {t("acp.sessions.searchEmpty")}
              </p>
            ) : null}
          </div>
        ) : (
          <div className="space-y-1">
            {groups.map((group) => {
              const open = isGroupOpen(group.key, group.containsCurrent, collapsedKeys);
              return (
                <GroupSection
                  key={group.key || UNGROUPED_KEY}
                  group={group}
                  groupLabel={group.key === UNGROUPED_KEY ? t("acp.sessions.ungrouped") : group.label}
                  open={open}
                  transientOpen={transientKeys.includes(group.key)}
                  activeSessionId={activeSessionId}
                  online={online}
                  onToggle={() => toggleGroup(group.key)}
                  onExpandHidden={() => expandGroupTransiently(group.key)}
                  onResume={(id) => void resumeSession(id)}
                  onClose={(id) => void closeSession(id)}
                />
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
