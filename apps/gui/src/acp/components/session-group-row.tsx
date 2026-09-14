// 两级树组头行（uix-spec I2/I4）：folder 开/闭两态、hover 换右向箭头随展开旋转
// 90°、含当前会话的 folder 变 info 蓝；折叠态超限显「展开 N 个」一次性全开。
import { ChevronRight, Folder, FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { sessionGroupTestId } from "@/acp/workspace-model";

export function SessionGroupRow(props: {
  groupKey: string;
  label: string;
  open: boolean;
  containsCurrent: boolean;
  hiddenCount: number;
  onToggle: () => void;
  onExpandHidden: () => void;
}) {
  const { t } = useTranslation();
  const testId = sessionGroupTestId(props.groupKey);
  return (
    <div>
      <button
        type="button"
        className={cn(
          "group/row flex h-[34px] w-full items-center gap-1.5 rounded-lg px-2 text-left",
          "hover:bg-accent",
        )}
        onClick={props.onToggle}
        aria-expanded={props.open}
        data-testid={`acp-group-row-${testId}`}
      >
        <span className="relative size-4 shrink-0">
          {props.open ? (
            <FolderOpen
              className={cn(
                "size-4 transition-opacity group-hover/row:opacity-0",
                props.containsCurrent && "text-info",
              )}
              aria-hidden
            />
          ) : (
            <Folder
              className={cn(
                "size-4 transition-opacity group-hover/row:opacity-0",
                props.containsCurrent && "text-info",
              )}
              aria-hidden
            />
          )}
          <ChevronRight
            className={cn(
              "absolute inset-0 size-4 text-muted-foreground opacity-0 transition-transform",
              "group-hover/row:opacity-100",
              props.open && "rotate-90",
            )}
            aria-hidden
          />
        </span>
        <span className="min-w-0 flex-1 truncate text-sm font-medium">{props.label}</span>
      </button>
      {props.open && props.hiddenCount > 0 ? (
        <Button
          variant="ghost"
          size="sm"
          className="text-muted-foreground h-7 w-full justify-start pl-9"
          onClick={props.onExpandHidden}
          data-testid={`acp-group-expand-${testId}`}
        >
          {t("acp.sessions.expandMore", { count: props.hiddenCount })}
        </Button>
      ) : null}
    </div>
  );
}
