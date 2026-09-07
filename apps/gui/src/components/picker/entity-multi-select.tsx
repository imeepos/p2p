import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { SearchIcon, XIcon } from "lucide-react";

import { Input } from "@/components/ui/input";

import { filterOptions, type PickerOption } from "./picker-option";
import { PickerOptionRow } from "./picker-option-row";
import { PickerStatusRow } from "./picker-status-row";

interface EntityMultiSelectProps {
  options: PickerOption[];
  selected: string[];
  onChange: (next: string[]) => void;
  disabled?: boolean;
  /** 数据面状态由调用方持有：加载/错误/重试三态与单选选择器同口径 */
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  /** 调用方口径的告警文案（如群成员上限），选择器只负责就地呈现 */
  warning?: string | null;
  warningTestId?: string;
  testId?: string;
  /** 搜索框 id（Label htmlFor 命中用）；搜索占位/空态文案非节点语境可覆盖 */
  id?: string;
  searchPlaceholder?: string;
  emptyText?: string;
}

// 统一多选关联选择器：即时搜索；已选区置顶（chip 可单个移除）+ 已选计数
// （aria-live 播报）；告警文案就地呈现。数据与上限口径由调用方注入。
export function EntityMultiSelect({
  options,
  selected,
  onChange,
  disabled,
  loading,
  error,
  onRetry,
  warning,
  warningTestId,
  testId = "entity-multi-select",
  id,
  searchPlaceholder,
  emptyText,
}: EntityMultiSelectProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listId = testId + "-list";

  const filtered = useMemo(() => filterOptions(options, query), [options, query]);
  const selectedOptions = selected
    .map((value) => options.find((option) => option.value === value))
    .filter((option): option is PickerOption => option !== undefined);
  // active 渲染期钳制（不落 effect）：列表缩短时高亮不越界
  const activeIndex = Math.min(active, Math.max(0, filtered.length - 1));

  const toggle = (value: string) => {
    onChange(
      selected.includes(value)
        ? selected.filter((item) => item !== value)
        : [...selected, value],
    );
  };

  const onInputKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((prev) => (filtered.length === 0 ? 0 : (prev + 1) % filtered.length));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((prev) =>
        filtered.length === 0 ? 0 : (prev - 1 + filtered.length) % filtered.length,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      const option = filtered[activeIndex];
      if (option) toggle(option.value);
    } else if (event.key === "Escape" && query.length > 0) {
      // 有查询词先清词且不穿透（避免连带关闭外层 Dialog）；无查询词放行给外层
      event.preventDefault();
      event.stopPropagation();
      setQuery("");
      setActive(0);
    }
  };

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <div className="relative">
        <SearchIcon
          aria-hidden
          className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
        />
        <Input
          id={id}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={onInputKeyDown}
          placeholder={searchPlaceholder ?? t("picker.searchPlaceholder")}
          role="combobox"
          aria-expanded
          aria-controls={listId}
          aria-activedescendant={filtered.length > 0 ? listId + "-opt-" + activeIndex : undefined}
          aria-autocomplete="list"
          autoComplete="off"
          className="pl-8"
          disabled={disabled}
          data-testid={testId + "-search"}
        />
      </div>
      {selectedOptions.length > 0 ? (
        <div
          className="flex flex-wrap items-center gap-1"
          aria-live="polite"
          data-testid={testId + "-selected"}
        >
          <span className="text-muted-foreground text-xs">
            {t("picker.selectedCount", { count: selected.length })}
          </span>
          {selectedOptions.map((option) => (
            <span
              key={option.value}
              className="bg-accent text-accent-foreground flex items-center gap-1 rounded px-1.5 py-0.5 text-xs"
            >
              {option.label}
              <button
                type="button"
                aria-label={t("picker.removeOne", { label: option.label })}
                data-testid={testId + "-remove-" + option.value}
                className="hover:text-destructive"
                disabled={disabled}
                onClick={() => toggle(option.value)}
              >
                <XIcon aria-hidden className="size-3" />
              </button>
            </span>
          ))}
        </div>
      ) : null}
      {warning ? (
        <p className="text-destructive text-xs" role="alert" data-testid={warningTestId}>
          {warning}
        </p>
      ) : null}
      <div
        role="listbox"
        aria-multiselectable
        id={listId}
        className="scroll-slim flex max-h-48 flex-col gap-0.5 overflow-y-auto rounded-md border p-1"
      >
        {loading || error ? (
          <PickerStatusRow loading={loading} error={error} onRetry={onRetry} emptyText={emptyText} />
        ) : filtered.length === 0 ? (
          <p className="text-muted-foreground px-2 py-1.5 text-sm" data-testid="picker-empty">
            {emptyText ?? t("picker.empty")}
          </p>
        ) : (
          filtered.map((option, index) => (
            <PickerOptionRow
              key={option.value}
              option={option}
              selected={selected.includes(option.value)}
              active={index === activeIndex}
              optionId={listId + "-opt-" + index}
              onHover={() => setActive(index)}
              onSelect={() => toggle(option.value)}
            />
          ))
        )}
      </div>
    </div>
  );
}
