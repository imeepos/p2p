// agent 会话段两级树（uix-spec §2 移植进 /chat）：分组/开合/限流全部复用
// acp/workspace-model 纯函数与 SessionGroupRow 组头，不复制逻辑。组键映射：
// AcpEndpoint 无 cwd，取条目 host（wsUrl 主机位）作工作区键——零数据面改动，
// 无 host 落未分组尾组。行仍为 ConversationRow，选中/右键/静音语义不变。
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { SessionGroupRow } from "@/acp/components/session-group-row";
import {
  COLLAPSED_SESSION_LIMIT,
  collapseGroupRows,
  groupSessionsByWorkspace,
  isGroupOpen,
  UNGROUPED_KEY,
  type WorkspaceGroupNode,
} from "@/acp/workspace-model";
import { useWorkspaceUiStore } from "@/acp/workspace-ui-store";
import type { SessionSummary } from "@/acp/protocol";
import { ConversationRow } from "@/components/chat/conversation-row";
import type { ContextMenuAnchor } from "@/components/ui/context-menu";
import type { ConversationEntry } from "@/lib/conversation-entry";

export interface AgentSessionTreeProps {
  /** agent 段条目（调用方已完成搜索过滤与排序） */
  entries: ConversationEntry[];
  selectedId: string | null;
  onSelect: (entry: ConversationEntry) => void;
  mutedOf: (entry: ConversationEntry) => boolean;
  onRowContextMenu: (entry: ConversationEntry, anchor: ContextMenuAnchor) => void;
}

/** agent 条目 → 分组纯函数入参投影：cwd 位填 host，sessionId/title 直传 */
function agentSessionRef(entry: ConversationEntry): SessionSummary {
  return { sessionId: entry.id, title: entry.title, cwd: entry.host ?? undefined };
}

function AgentGroupSection(props: {
  group: WorkspaceGroupNode;
  groupLabel: string;
  open: boolean;
  transientOpen: boolean;
  entryOf: (sessionId: string) => ConversationEntry | undefined;
  selectedId: string | null;
  onSelect: (entry: ConversationEntry) => void;
  mutedOf: (entry: ConversationEntry) => boolean;
  onRowContextMenu: (entry: ConversationEntry, anchor: ContextMenuAnchor) => void;
  onToggle: () => void;
  onExpandHidden: () => void;
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
      {props.open ? (
        <ul className="space-y-0.5 pl-2">
          {rows.map((ref) => {
            const entry = props.entryOf(ref.sessionId);
            // 组由同一 entries 数组派生，缺项仅防御性跳过
            if (!entry) return null;
            return (
              <ConversationRow
                key={entry.kind + ":" + entry.id}
                entry={entry}
                active={entry.id === props.selectedId}
                onSelect={props.onSelect}
                muted={props.mutedOf(entry)}
                onContextMenu={(event) => {
                  // 阻止浏览器默认菜单与外层关闭监听（与扁平路径同纪律）
                  event.preventDefault();
                  event.stopPropagation();
                  props.onRowContextMenu(entry, { x: event.clientX, y: event.clientY });
                }}
              />
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}

export function AgentSessionTree({
  entries,
  selectedId,
  onSelect,
  mutedOf,
  onRowContextMenu,
}: AgentSessionTreeProps) {
  const { t } = useTranslation();
  const collapsedKeys = useWorkspaceUiStore((s) => s.collapsedGroupKeys);
  const transientKeys = useWorkspaceUiStore((s) => s.transientExpandedKeys);
  const toggleGroup = useWorkspaceUiStore((s) => s.toggleGroup);
  const expandGroupTransiently = useWorkspaceUiStore((s) => s.expandGroupTransiently);

  const refs = useMemo(() => entries.map(agentSessionRef), [entries]);
  const groups = useMemo(
    () => groupSessionsByWorkspace(refs, selectedId),
    [refs, selectedId],
  );
  const entryOf = useMemo(() => {
    const byId = new Map(entries.map((entry) => [entry.id, entry]));
    return (sessionId: string) => byId.get(sessionId);
  }, [entries]);

  return (
    <div
      data-testid="agent-session-tree"
      aria-label={t("chat.conversations.agentSection")}
      className="space-y-1"
    >
      {groups.map((group) => {
        const open = isGroupOpen(group.key, group.containsCurrent, collapsedKeys);
        return (
          <AgentGroupSection
            key={group.key || UNGROUPED_KEY}
            group={group}
            groupLabel={group.key === UNGROUPED_KEY ? t("acp.sessions.ungrouped") : group.label}
            open={open}
            transientOpen={transientKeys.includes(group.key)}
            entryOf={entryOf}
            selectedId={selectedId}
            onSelect={onSelect}
            mutedOf={mutedOf}
            onRowContextMenu={onRowContextMenu}
            onToggle={() => toggleGroup(group.key)}
            onExpandHidden={() => expandGroupTransiently(group.key)}
          />
        );
      })}
    </div>
  );
}
