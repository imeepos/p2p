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

// WX1 聊天时间用 HH:mm（微信桌面同款短时间），与日志/遥测的 medium 区分
export function formatTimeShort(epochMs: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(
    epochMs,
  );
}

// 聊天会话时间（W1-03）：今天显 HH:mm，昨天显「昨天 HH:mm」，更早带日期。
// WX1 统一短时间时砍掉日期维度，隔天会话无法区分日期，这里按日历日回补。
export function formatConversationTime(epochMs: number, locale: Locale): string {
  const time = formatTimeShort(epochMs, locale);
  const startOfDay = (d: Date) =>
    new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const dayDiff = Math.round((startOfDay(new Date()) - startOfDay(new Date(epochMs))) / 86400000);
  if (dayDiff === 0) return time;
  if (dayDiff === 1) {
    return (
      new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(-1, "day") +
      " " +
      time
    );
  }
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(epochMs);
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
