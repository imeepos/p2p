import { useTranslation } from "react-i18next";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

// 节头（P2#8）：标题居左；检索框 + 「匹配/总数」计数 + 添加按钮居右。
// 检索词为各节内部 state，不跨节共享。
export function SectionHeader(props: {
  id: string;
  title: string;
  query: string;
  onQueryChange: (query: string) => void;
  placeholder: string;
  matched: number;
  total: number;
  addLabel: string;
  addIcon: LucideIcon;
  onAdd: () => void;
  addTestId: string;
}) {
  const { t } = useTranslation();
  const AddIcon = props.addIcon;
  return (
    <div className="flex items-center justify-between gap-2">
      <h2 className="text-sm font-semibold">{props.title}</h2>
      <div className="flex items-center gap-2">
        <Input
          value={props.query}
          onChange={(event) => props.onQueryChange(event.target.value)}
          placeholder={props.placeholder}
          aria-label={props.placeholder}
          className="h-8 w-40 text-xs"
          autoComplete="off"
          data-testid={"contacts-search-" + props.id}
        />
        <span
          className="text-muted-foreground text-xs"
          data-testid={"contacts-count-" + props.id}
        >
          {t("contacts.countOf", { matched: props.matched, total: props.total })}
        </span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={props.onAdd}
          data-testid={props.addTestId}
        >
          <AddIcon aria-hidden className="size-4" />
          {props.addLabel}
        </Button>
      </div>
    </div>
  );
}
