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
  clearEvents: () => Promise<void>;
  exportJson: () => void;
}

export function useEventsController(): EventsController {
  const live = useNodeStore((s) => s.events);
  const subscriptionLive = useNodeStore((s) => s.subscriptionLive);
  const [paused, setPaused] = useState(false);
  const [snapshot, setSnapshot] = useState<NodeEventJson[] | null>(null);
  const [query, setQuery] = useState("");
  const [errorOnly, setErrorOnly] = useState(false);
  const [typeFilter, setTypeFilter] = useState<ReadonlySet<NodeEventType>>(
    () => new Set(ALL_EVENT_TYPES),
  );

  const events = snapshot ?? live;
  const newCount = Math.max(0, live.length - (snapshot?.length ?? 0));
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
      setSnapshot(live);
      setPaused(true);
    }
  }, [paused, live]);

  const commands = useEventsCommands({
    live,
    paused,
    filtered,
    onSnapshotClear: () => setSnapshot([]),
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
    ...commands,
  };
}
