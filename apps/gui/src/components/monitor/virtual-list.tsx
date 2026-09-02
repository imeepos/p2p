import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { cn } from "@/lib/utils";

export interface VirtualListProps<T> {
  items: T[];
  heightAt: (item: T, index: number) => number;
  renderItem: (item: T, index: number) => ReactNode;
  className?: string;
  overscan?: number;
}

function findIndexAtOffset(offsets: number[], offset: number): number {
  let lo = 0;
  let hi = offsets.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (offsets[mid] <= offset) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

interface WindowRange {
  offsets: number[];
  start: number;
  end: number;
}

function useWindowRange(
  itemCount: number,
  heightAt: (index: number) => number,
  scrollTop: number,
  viewportH: number,
  overscan: number,
): WindowRange {
  return useMemo(() => {
    const offsets = new Array<number>(itemCount + 1);
    offsets[0] = 0;
    for (let i = 0; i < itemCount; i += 1) {
      offsets[i + 1] = offsets[i] + heightAt(i);
    }
    const total = offsets[itemCount];
    const clamped = Math.min(scrollTop, Math.max(0, total - viewportH));
    const start = Math.max(0, findIndexAtOffset(offsets, clamped) - overscan);
    let end = start;
    while (end < itemCount && offsets[end] < clamped + viewportH) end += 1;
    return { offsets, start, end: Math.min(itemCount, end + overscan) };
  }, [itemCount, heightAt, scrollTop, viewportH, overscan]);
}

// 轻量窗口化列表：前缀和 + 二分定位，无第三方依赖；行高由调用方给定。
export function VirtualList<T>({
  items,
  heightAt,
  renderItem,
  className,
  overscan = 4,
}: VirtualListProps<T>) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(0);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setViewportH(el.clientHeight));
    observer.observe(el);
    setViewportH(el.clientHeight);
    return () => observer.disconnect();
  }, []);

  const onScroll = useCallback(() => {
    const el = containerRef.current;
    if (el) setScrollTop(el.scrollTop);
  }, []);

  const { offsets, start, end } = useWindowRange(
    items.length,
    (index) => heightAt(items[index], index),
    scrollTop,
    viewportH,
    overscan,
  );

  const rows: ReactNode[] = [];
  for (let i = start; i < end; i += 1) {
    rows.push(
      <div
        key={i}
        style={{ height: offsets[i + 1] - offsets[i] }}
        className="overflow-hidden"
      >
        {renderItem(items[i], i)}
      </div>,
    );
  }

  return (
    <div
      ref={containerRef}
      onScroll={onScroll}
      className={cn("overflow-y-auto", className)}
    >
      <div className="relative w-full" style={{ height: offsets[items.length] ?? 0 }}>
        <div
          className="absolute inset-x-0 top-0"
          style={{ transform: `translateY(${offsets[start] ?? 0}px)` }}
        >
          {rows}
        </div>
      </div>
    </div>
  );
}
