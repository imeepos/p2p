import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDownIcon, ChevronRightIcon } from "lucide-react";

import { cn } from "@/lib/utils";

interface TreeSectionProps {
  id: string;
  title: string;
  expanded: boolean;
  onToggle: () => void;
  toggleTestId: string;
  active?: boolean;
  onGo?: () => void;
  anchorTestId?: string;
  anchorLabel?: string;
  wrapperTestId?: string;
  count?: ReactNode;
  badge?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}

// 左栏树分节（微信通讯录式）：折叠箭头 + 标题（可作锚点）+ 徽标/计数 +
// 行内动作；体内容仅展开时渲染。标题锚点点击交给 onGo（页面级滚动定位
// 与当前节高亮），折叠只归箭头钮，两者互不干扰。
export function TreeSection(props: TreeSectionProps) {
  const { t } = useTranslation();
  return (
    <section
      id={props.id}
      data-testid={props.wrapperTestId}
      aria-label={props.title}
      className="flex flex-col"
    >
      <div className="flex items-center gap-1 px-1 py-1">
        <button
          type="button"
          data-testid={props.toggleTestId}
          aria-expanded={props.expanded}
          aria-label={t("contacts.tree.toggle")}
          onClick={props.onToggle}
          className="text-muted-foreground hover:text-foreground flex size-6 shrink-0 items-center justify-center rounded hover:bg-accent"
        >
          {props.expanded ? (
            <ChevronDownIcon aria-hidden className="size-4" />
          ) : (
            <ChevronRightIcon aria-hidden className="size-4" />
          )}
        </button>
        {props.anchorTestId ? (
          <button
            type="button"
            data-testid={props.anchorTestId}
            data-active={props.active ? "true" : "false"}
            aria-label={props.anchorLabel}
            aria-current={props.active ? "true" : undefined}
            onClick={props.onGo}
            className={cn(
              "hover:bg-accent min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left text-sm font-semibold",
              props.active && "font-bold",
            )}
          >
            {props.title}
          </button>
        ) : (
          <span className="min-w-0 flex-1 truncate px-1 text-sm font-semibold">{props.title}</span>
        )}
        {props.badge}
        {props.count}
        {props.actions}
      </div>
      {props.expanded ? <div className="flex flex-col gap-0.5 pb-2">{props.children}</div> : null}
    </section>
  );
}
