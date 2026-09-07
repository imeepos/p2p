import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { DialHopKind, NodeEventJson, NodeEventType } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { describeNodeEvent } from "@/lib/event-text";
import { usePeerNameLabel } from "@/lib/peer-name";
import { ALL_EVENT_TYPES, eventSummary } from "@/views/network/event-meta";
import { HOP_KEY } from "@/views/network/hop-labels";
import { toLooseT } from "@/views/network/loose-t";
import { countEventsByType } from "./event-filter-groups";
import { filterEvents } from "./events-filter";
import { useEventsCommands } from "./use-events-commands";

export interface EventsController {
  events: NodeEventJson[];
  filtered: NodeEventJson[];
  /** F20：缓冲区按类型命中计数（空缓冲全 0），供筛选 chip 展示。 */
  counts: Record<NodeEventType, number>;
  subscriptionLive: boolean;
  paused: boolean;
  newCount: number;
  query: string;
  setQuery: (query: string) => void;
  errorOnly: boolean;
  setErrorOnly: (value: boolean) => void;
  typeFilter: ReadonlySet<NodeEventType>;
  toggleType: (type: NodeEventType) => void;
  togglePause: () => void;
  resetFilters: () => void;
  clearEvents: () => Promise<void>;
  exportJson: () => void;
}

export function useEventsController(): EventsController {
  const live = useNodeStore((s) => s.events);
  const subscriptionLive = useNodeStore((s) => s.subscriptionLive);
  const eventSeq = useNodeStore((s) => s.eventSeq);
  const [paused, setPaused] = useState(false);
  const [snapshot, setSnapshot] = useState<NodeEventJson[] | null>(null);
  // 暂停起点的事件序号：新增计数 = eventSeq - pausedSeq。环形缓冲打满后
  // live.length 恒为 MAX_EVENTS，旧的「长度差」公式会停在错误小数字甚至
  // 0，误导用户以为没有新事件而放心挂机。
  const [pausedSeq, setPausedSeq] = useState(0);
  const [query, setQuery] = useState("");
  const [errorOnly, setErrorOnly] = useState(false);
  const [typeFilter, setTypeFilter] = useState<ReadonlySet<NodeEventType>>(
    () => new Set(ALL_EVENT_TYPES),
  );

  const events = snapshot ?? live;
  const newCount = paused ? Math.max(0, eventSeq - pausedSeq) : 0;

  // N1 搜索同源化：语料 = 行内所见人话摘要（短形态）+ 全量形态（完整
  // PeerId 可搜）+ 原始负载摘要（reason/addr 等内部字段兜底）。
  const { t } = useTranslation();
  const tt = toLooseT(t);
  const peerLabel = usePeerNameLabel();
  const searchText = useCallback(
    (event: NodeEventJson) => {
      const labels = {
        hopLabel: (kind: DialHopKind) => tt(HOP_KEY[kind]),
        okLabel: tt("events.outcome.ok"),
        failLabel: tt("events.outcome.fail"),
      };
      const short = eventSummary(event, labels, { peerLabel });
      const full = eventSummary(event, labels, { full: true });
      return [
        tt(short.key, short.values),
        tt(full.key, full.values),
        describeNodeEvent(event),
      ].join("\n");
    },
    [tt, peerLabel],
  );

  const filtered = useMemo(
    () => filterEvents(events, { query, errorOnly, typeFilter, searchText }),
    [events, query, errorOnly, typeFilter, searchText],
  );
  const counts = useMemo(() => countEventsByType(events), [events]);

  const toggleType = useCallback((type: NodeEventType) => {
    setTypeFilter((prev) => {
      const next = new Set(prev);
      if (next.has(type)) next.delete(type);
      else next.add(type);
      return next;
    });
  }, []);

  const togglePause = useCallback(() => {
    if (paused) {
      setSnapshot(null);
      setPaused(false);
    } else {
      // 点击瞬间从 store 取最新切片，避免闭包里的 live 落后一拍。
      const { events: liveEvents, eventSeq: seq } = useNodeStore.getState();
      setSnapshot(liveEvents);
      setPausedSeq(seq);
      setPaused(true);
    }
  }, [paused]);

  const resetFilters = useCallback(() => {
    setQuery("");
    setErrorOnly(false);
    setTypeFilter(new Set(ALL_EVENT_TYPES));
  }, []);

  const commands = useEventsCommands({
    live,
    paused,
    filtered,
    onSnapshotClear: () => {
      // 暂停期间清空：快照置空且计数从清空点重新起算，不把已丢弃的旧事件算进「新增」。
      setSnapshot([]);
      setPausedSeq(useNodeStore.getState().eventSeq);
    },
  });

  return {
    events,
    filtered,
    counts,
    subscriptionLive,
    paused,
    newCount,
    query,
    setQuery,
    errorOnly,
    setErrorOnly,
    typeFilter,
    toggleType,
    togglePause,
    resetFilters,
    ...commands,
  };
}
