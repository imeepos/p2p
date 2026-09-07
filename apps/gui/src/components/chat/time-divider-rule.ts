// WX1 居中时间分割线判定：与上一条间隔 ≥5min 或为首条时插入。
export const TIME_DIVIDER_GAP_MS = 5 * 60 * 1000;

export function needsTimeDivider(
  prev: { tsMs: number } | null,
  current: { tsMs: number },
): boolean {
  if (prev === null) return true;
  return current.tsMs - prev.tsMs >= TIME_DIVIDER_GAP_MS;
}
