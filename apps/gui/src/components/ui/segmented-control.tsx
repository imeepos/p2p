import { cn } from "@/lib/utils";

export interface SegmentedOption<V extends string> {
  value: V;
  label: string;
  /** 可选计数（如待处理条数），渲染在标签右侧等宽数字 */
  count?: number;
}

interface SegmentedControlProps<V extends string> {
  value: V;
  onChange: (value: V) => void;
  options: readonly SegmentedOption<V>[];
  ariaLabel: string;
  className?: string;
}

// macOS 风格分段控件（视图切换原语）。WAI-ARIA APG：纯视图过滤场景无面板
// 关联，用按钮组 + aria-pressed 比完整 tablist 语义更贴切（peers-toolbar
// chips 同口径）；选中态 bg-background + shadow 浮起，未选中 muted。
export function SegmentedControl<V extends string>({
  value,
  onChange,
  options,
  ariaLabel,
  className,
}: SegmentedControlProps<V>) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={cn(
        "bg-muted inline-flex items-center gap-0.5 rounded-md p-0.5",
        className,
      )}
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            data-testid={"segmented-" + option.value}
            onClick={() => onChange(option.value)}
            className={cn(
              "rounded-[5px] px-3 py-1 text-xs font-medium whitespace-nowrap transition-colors",
              selected
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {option.label}
            {typeof option.count === "number" ? (
              <span className="ml-1 tabular-nums">{option.count}</span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
