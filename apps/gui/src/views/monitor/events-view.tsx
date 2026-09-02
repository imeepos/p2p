import { useTranslation } from "react-i18next";
import { useCallback, useState } from "react";

import { PageHeader } from "@/components/page/page-header";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { useEventsController } from "./use-events-controller";
import { EventsFilterBar } from "./events-filter-bar";
import { EventsActionsBar } from "./events-actions-bar";
import { EventsListCard } from "./events-list-card";
import { EVENT_ROW_EXPANDED_HEIGHT, EVENT_ROW_HEIGHT } from "./event-row";

export function EventsView() {
  const { i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const controller = useEventsController();
  const [expanded, setExpanded] = useState<ReadonlySet<NodeEventJson>>(
    () => new Set(),
  );

  const heightAt = useCallback(
    (event: NodeEventJson) =>
      expanded.has(event) ? EVENT_ROW_EXPANDED_HEIGHT : EVENT_ROW_HEIGHT,
    [expanded],
  );

  const toggleExpanded = useCallback((event: NodeEventJson) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(event)) next.delete(event);
      else next.add(event);
      return next;
    });
  }, []);

  return (
    <>
      <PageHeader titleKey="events.title" descriptionKey="events.description" />

      <EventsFilterBar
        query={controller.query}
        onQueryChange={controller.setQuery}
        errorOnly={controller.errorOnly}
        onErrorOnlyChange={controller.setErrorOnly}
        typeFilter={controller.typeFilter}
        onToggleType={controller.toggleType}
      />
      <EventsActionsBar
        paused={controller.paused}
        newCount={controller.newCount}
        onTogglePause={controller.togglePause}
        onExport={controller.exportJson}
        exportDisabled={controller.filtered.length === 0}
        onClear={() => void controller.clearEvents()}
        clearDisabled={controller.events.length === 0}
      />

      <EventsListCard
        loading={!controller.subscriptionLive}
        bufferEmpty={controller.events.length === 0}
        filtered={controller.filtered}
        locale={locale}
        expanded={expanded}
        heightAt={heightAt}
        onToggle={toggleExpanded}
      />
    </>
  );
}
