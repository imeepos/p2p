import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDownIcon, Loader2Icon, XIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

import { filterOptions, type PickerOption } from "./picker-option";
import { PickerOptionRow } from "./picker-option-row";

interface EntityComboboxProps {
  options: PickerOption[];
  value: string | null;
  onChange: (value: string | null) => void;
  /** 数据面状态由调用方持有（store 在视图层）：加载/错误/重试三态 */
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  disabled?: boolean;
  id?: string;
  testId?: string;
}

function StatusRow({
  loading,
  error,
  onRetry,
}: {
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
}) {
  const { t } = useTranslation();
  if (loading) {
    return (
      <p
        className="text-muted-foreground flex items-center gap-2 px-2 py-1.5 text-sm"
        data-testid="picker-loading"
        role="status"
      >
        <Loader2Icon aria-hidden className="size-4 animate-spin" />
        {t("picker.loading")}
      </p>
    );
  }
  if (error) {
    return (
      <div
        className="flex flex-col gap-1 px-2 py-1.5"
        data-testid="picker-error"
        role="alert"
      >
        <p className="text-destructive text-sm">{t("picker.loadFailed")}</p>
        {onRetry ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-fit"
            onClick={onRetry}
            data-testid="picker-retry"
          >
            {t("picker.retry")}
          </Button>
        ) : null}
      </div>
    );
  }
  return (
    <p className="text-muted-foreground px-2 py-1.5 text-sm" data-testid="picker-empty">
      {t("picker.empty")}
    </p>
  );
}

// 统一单选关联选择器：触发器与表单控件同款，展开即时搜索 + 键盘上下/回车，
// 可清空；加载/错误/空三态内嵌。数据（选项、三态）由调用方注入。
export function EntityCombobox({
  options,
  value,
  onChange,
  loading,
  error,
  onRetry,
  disabled,
  id = "entity-combobox",
  testId = "entity-combobox",
}: EntityComboboxProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = id + "-list";

  const filtered = useMemo(() => filterOptions(options, query), [options, query]);
  const selected = options.find((option) => option.value === value) ?? null;
  const activeId = listId + "-opt-" + active;

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  useEffect(() => {
    setActive((prev) => Math.min(prev, Math.max(0, filtered.length - 1)));
  }, [filtered.length]);

  const openPanel = () => {
    setQuery("");
    setActive(0);
    setOpen(true);
    requestAnimationFrame(() => inputRef.current?.focus());
  };

  const pick = (option: PickerOption | undefined) => {
    if (!option) return;
    onChange(option.value);
    setOpen(false);
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
      pick(filtered[active]);
    } else if (event.key === "Escape") {
      setOpen(false);
    }
  };

  return (
    <div ref={rootRef} className="relative w-full">
      <Button
        type="button"
        variant="outline"
        role="combobox"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={open ? listId : undefined}
        disabled={disabled}
        className={cn("h-9 w-full justify-between px-3 font-normal", open && "border-ring ring-ring/50 ring-[3px]")}
        onClick={() => (open ? setOpen(false) : openPanel())}
        data-testid={testId}
        data-state={open ? "open" : "closed"}
      >
        <span className="flex min-w-0 flex-col items-start">
          <span className={cn("truncate text-sm", !selected && "text-muted-foreground")}>
            {selected ? selected.label : t("picker.triggerPlaceholder")}
          </span>
          {selected?.hint ? (
            <span className="text-muted-foreground font-mono text-xs">{selected.hint}</span>
          ) : null}
        </span>
        {selected ? (
          <span
            role="button"
            tabIndex={0}
            aria-label={t("picker.clear")}
            data-testid={testId + "-clear"}
            className="text-muted-foreground hover:text-foreground shrink-0 rounded-sm p-0.5"
            onClick={(event) => {
              event.stopPropagation();
              onChange(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.stopPropagation();
                onChange(null);
              }
            }}
          >
            <XIcon aria-hidden className="size-4" />
          </span>
        ) : (
          <ChevronDownIcon aria-hidden className="size-4 shrink-0 opacity-50" />
        )}
      </Button>
      {open ? (
        <div
          className="bg-popover text-popover-foreground absolute z-50 mt-1 w-full rounded-md border shadow-md"
          data-testid={testId + "-panel"}
        >
          <div className="p-2 pb-0">
            <Input
              ref={inputRef}
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                setActive(0);
              }}
              onKeyDown={onInputKeyDown}
              placeholder={t("picker.searchPlaceholder")}
              role="combobox"
              aria-expanded
              aria-controls={listId}
              aria-activedescendant={filtered.length > 0 ? activeId : undefined}
              aria-autocomplete="list"
              autoComplete="off"
              data-testid={testId + "-search"}
            />
          </div>
          <div role="listbox" id={listId} aria-label={t("picker.triggerPlaceholder")} className="scroll-slim max-h-60 overflow-y-auto p-1">
            {loading || error || filtered.length === 0 ? (
              <StatusRow loading={loading} error={error} onRetry={onRetry} />
            ) : (
              filtered.map((option, index) => (
                <PickerOptionRow
                  key={option.value}
                  option={option}
                  selected={option.value === value}
                  active={index === active}
                  optionId={listId + "-opt-" + index}
                  onHover={() => setActive(index)}
                  onSelect={() => pick(option)}
                />
              ))
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
