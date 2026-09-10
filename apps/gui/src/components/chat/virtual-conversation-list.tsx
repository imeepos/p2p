import { useCallback, useMemo } from "react";
import { List, type RowComponentProps } from "react-window";

import { ConversationRow } from "@/components/chat/conversation-row";
import { InvitePlaceholderRow } from "@/components/chat/invite-placeholder-row";
import type { ContextMenuAnchor } from "@/components/ui/context-menu";
import type { ConversationEntry } from "@/lib/conversation-entry";
import type { PendingInviteItem } from "@/views/chat/use-pending-invites";

// 会话列表虚拟路径（react-window 定高行）：行高固定 64px，DOM 节点数与
// 会话总量解耦。短列表（≤阈值）仍走普通 ul 渲染，行为语义不变。

export const CONVERSATION_VIRTUAL_THRESHOLD = 100;
const ROW_HEIGHT = 64;
const OVERSCAN = 8;

type ConversationRowModel =
  | { type: "entry"; key: string; entry: ConversationEntry }
  | { type: "invite"; key: string; item: PendingInviteItem };

export interface ConversationVirtualRowProps {
  rows: ConversationRowModel[];
  selectedId: string | null;
  onSelect: (entry: ConversationEntry) => void;
  mutedOf: (entry: ConversationEntry) => boolean;
  onRowContextMenu: (entry: ConversationEntry, anchor: ContextMenuAnchor) => void;
}

function ConversationVirtualRow({
  index,
  style,
  rows,
  selectedId,
  onSelect,
  mutedOf,
  onRowContextMenu,
}: RowComponentProps<ConversationVirtualRowProps>) {
  const row = rows[index];
  return (
    <div style={style}>
      {row.type === "entry" ? (
        <ConversationRow
          entry={row.entry}
          active={row.entry.id === selectedId}
          onSelect={onSelect}
          muted={mutedOf(row.entry)}
          onContextMenu={(event) => {
            // 阻止浏览器默认菜单与外层关闭监听（stopPropagation 截断冒泡）
            event.preventDefault();
            event.stopPropagation();
            onRowContextMenu(row.entry, { x: event.clientX, y: event.clientY });
          }}
        />
      ) : (
        <InvitePlaceholderRow item={row.item} />
      )}
    </div>
  );
}

export interface VirtualConversationListProps {
  entries: ConversationEntry[];
  pendingInvites: PendingInviteItem[];
  selectedId: string | null;
  onSelect: (entry: ConversationEntry) => void;
  mutedOf: (entry: ConversationEntry) => boolean;
  onRowContextMenu: (entry: ConversationEntry, anchor: ContextMenuAnchor) => void;
}

export function VirtualConversationList({
  entries,
  pendingInvites,
  selectedId,
  onSelect,
  mutedOf,
  onRowContextMenu,
}: VirtualConversationListProps) {
  const rows = useMemo<ConversationRowModel[]>(
    () => [
      ...entries.map((entry) => ({
        type: "entry" as const,
        key: entry.kind + ":" + entry.id,
        entry,
      })),
      ...pendingInvites.map((item) => ({
        type: "invite" as const,
        key: `invite:${item.kind}:${item.id}`,
        item,
      })),
    ],
    [entries, pendingInvites],
  );

  const rowKey = useCallback((index: number) => rows[index].key, [rows]);
  const rowProps = useMemo(
    () => ({ rows, selectedId, onSelect, mutedOf, onRowContextMenu }),
    [rows, selectedId, onSelect, mutedOf, onRowContextMenu],
  );

  return (
    <div
      className="scroll-slim min-h-0 flex-1 overflow-y-auto"
      data-testid="conversation-items"
    >
      <List
        rowComponent={ConversationVirtualRow}
        rowCount={rows.length}
        rowHeight={ROW_HEIGHT}
        rowProps={rowProps}
        rowKey={rowKey}
        overscanCount={OVERSCAN}
        style={{ height: "100%" }}
      />
    </div>
  );
}
