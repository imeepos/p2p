import type { Locale } from "@/i18n";

// F18：相对时间下限文案。放在 lib 层与 format.ts 的单位表同口径
// （lib 属契约层，豁免展示层硬编码扫描）。
const JUST_NOW: Record<Locale, string> = {
  "zh-CN": "刚刚",
  "en-US": "just now",
};

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

export function justNowLabel(locale: Locale): string {
  return JUST_NOW[locale] ?? JUST_NOW["en-US"];
}

// 相对时间只向后看：epochMs 晚于 now（clock skew/事件乱序）钳到当前，
// 一律落「刚刚」下限，杜绝「N秒钟后」类未来时表述（F18）。
export function formatRelative(
  epochMs: number,
  locale: Locale,
  now = Date.now(),
): string {
  const elapsedSec = Math.round((now - Math.min(epochMs, now)) / 1000);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (elapsedSec <= 0) return justNowLabel(locale);
  for (const { limit, unit } of LIMITS) {
    if (elapsedSec < limit) {
      const value = Math.max(1, Math.round(elapsedSec / (DIVISOR[unit] ?? 1)));
      return rtf.format(-value, unit);
    }
  }
  return rtf.format(-elapsedSec, "day");
}
