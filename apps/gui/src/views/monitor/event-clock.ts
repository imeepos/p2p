import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";

type WithTsMs = { tsMs?: number };

const receivedAt = new WeakMap<NodeEventJson, number>();
let armed = false;

function stamp(event: NodeEventJson, now: number): void {
  if (receivedAt.has(event)) return;
  // 契约 v1 §2：变体可带 tsMs；缺省用本地接收时间兜底。
  receivedAt.set(event, (event as WithTsMs).tsMs ?? now);
}

function arm(): void {
  if (armed) return;
  armed = true;
  useNodeStore.subscribe((state, prev) => {
    if (state.events === prev.events) return;
    const now = Date.now();
    for (const event of state.events) {
      // 新事件总在队首，遇到已记录的即可停。
      if (receivedAt.has(event)) break;
      stamp(event, now);
    }
  });
}

// 模块加载即挂订阅：store 收到事件的瞬间记录接收时间。
arm();

export function eventTimeMs(event: NodeEventJson): number {
  const known = receivedAt.get(event);
  if (known !== undefined) return known;
  const now = Date.now();
  stamp(event, now);
  return now;
}

const DIVISOR: Partial<Record<Intl.RelativeTimeFormatUnit, number>> = {
  second: 1,
  minute: 60,
  hour: 3600,
  day: 86400,
};

const LIMITS: Array<{ limit: number; unit: Intl.RelativeTimeFormatUnit }> = [
  { limit: 60, unit: "second" },
  { limit: 3600, unit: "minute" },
  { limit: 86400, unit: "hour" },
  { limit: Number.POSITIVE_INFINITY, unit: "day" },
];

export function formatRelative(
  epochMs: number,
  locale: Locale,
  now = Date.now(),
): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const diffSec = Math.round((epochMs - now) / 1000);
  const absSec = Math.abs(diffSec);
  for (const { limit, unit } of LIMITS) {
    if (absSec < limit) {
      return rtf.format(Math.round(diffSec / (DIVISOR[unit] ?? 1)), unit);
    }
  }
  return rtf.format(diffSec, "day");
}
