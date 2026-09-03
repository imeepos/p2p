import type { Locale } from "@/i18n";

const UPTIME_UNITS = {
  "zh-CN": { d: "天", h: "小时", m: "分", s: "秒" },
  "en-US": { d: "d", h: "h", m: "m", s: "s" },
} as const;

const BYTE_UNITS = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte"];

export function formatUptime(totalSeconds: number, locale: Locale): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const units = UPTIME_UNITS[locale] ?? UPTIME_UNITS["en-US"];
  if (days > 0) return `${days}${units.d} ${hours}${units.h}`;
  if (hours > 0) return `${hours}${units.h} ${minutes}${units.m}`;
  if (minutes > 0) return `${minutes}${units.m} ${rest}${units.s}`;
  return `${rest}${units.s}`;
}

export function formatTime(epochMs: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { timeStyle: "medium" }).format(
    epochMs,
  );
}

export function formatDateTime(epochMs: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(epochMs);
}

export function formatNumber(value: number, locale: Locale): string {
  return new Intl.NumberFormat(locale).format(value);
}

export function formatBytes(bytes: number, locale: Locale): string {
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: BYTE_UNITS[unitIndex],
    unitDisplay: "long",
    maximumFractionDigits: 1,
  }).format(value);
}
