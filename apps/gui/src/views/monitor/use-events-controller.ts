import { useCallback, useMemo, useState } from "react";
import type { NodeEventJson, NodeEventType } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { ALL_EVENT_TYPES } from "./event-meta";
import { filterEvents } from "./events-filter";
import { useEventsCommands } from "./use-events-commands";

export interface EventsController {
  events: NodeEventJson[];
  filtered: NodeEventJson[];
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
  const filtered = useMemo(
    () => filterEvents(events, { query, errorOnly, typeFilter }),
    [events, query, errorOnly, typeFilter],
  );

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
