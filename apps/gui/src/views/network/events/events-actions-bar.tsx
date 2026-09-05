import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { DownloadIcon, PauseIcon, PlayIcon, Trash2Icon } from "lucide-react";
import { Badge } from "@/components/ui/badge";

interface EventsActionsBarProps {
  paused: boolean;
  newCount: number;
  onTogglePause: () => void;
  onExport: () => void;
  exportDisabled: boolean;
  onClear: () => void;
  clearDisabled: boolean;
}

// 控制条：暂停/恢复（含暂停期间新增计数）、导出 JSON、清空。
export function EventsActionsBar({
  paused,
  newCount,
  onTogglePause,
  onExport,
  exportDisabled,
  onClear,
  clearDisabled,
}: EventsActionsBarProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12 flex items-center justify-end gap-2">
      {paused && (
        <Badge variant="secondary">
          {t("events.controls.pausedTip", { count: newCount })}
        </Badge>
      )}
      <Button size="sm" variant="outline" onClick={onTogglePause}>
        {paused ? <PlayIcon aria-hidden /> : <PauseIcon aria-hidden />}
        {paused ? t("events.controls.resume") : t("events.controls.pause")}
      </Button>
      <Button
        size="sm"
        variant="outline"
        onClick={onExport}
        disabled={exportDisabled}
      >
        <DownloadIcon aria-hidden />
        {t("events.controls.export")}
      </Button>
      <Button
        size="sm"
        variant="destructive"
        onClick={onClear}
        disabled={clearDisabled}
      >
        <Trash2Icon aria-hidden />
        {t("events.controls.clear")}
      </Button>
    </div>
  );
}
