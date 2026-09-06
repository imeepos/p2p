import { CheckIcon } from "lucide-react";

import { cn } from "@/lib/utils";

import type { PickerOption } from "./picker-option";

interface PickerOptionRowProps {
  option: PickerOption;
  selected: boolean;
  active: boolean;
  optionId: string;
  onHover: () => void;
  onSelect: () => void;
}

// 单行选项：人可读名主行 + 缩略 PeerId 副行；选中态打勾，active 高亮。
// mousedown preventDefault 保住搜索框焦点（点击选项不丢键盘上下文）。
export function PickerOptionRow({
  option,
  selected,
  active,
  optionId,
  onHover,
  onSelect,
}: PickerOptionRowProps) {
  return (
    <button
      type="button"
      role="option"
      id={optionId}
      aria-selected={selected}
      data-testid={optionId}
      className={cn(
        "flex w-full items-start gap-2 rounded-sm px-2 py-1.5 text-left text-sm outline-none",
        active && "bg-accent text-accent-foreground",
      )}
      onMouseDown={(event) => event.preventDefault()}
      onMouseEnter={onHover}
      onClick={onSelect}
    >
      <CheckIcon
        aria-hidden
        className={cn("mt-0.5 size-4 shrink-0", selected ? "opacity-100" : "opacity-0")}
      />
      <span className="flex min-w-0 flex-col">
        <span className="truncate">{option.label}</span>
        {option.hint ? (
          <span className="text-muted-foreground font-mono text-xs">{option.hint}</span>
        ) : null}
      </span>
    </button>
  );
}
