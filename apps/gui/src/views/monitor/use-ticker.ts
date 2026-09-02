import { useEffect, useState } from "react";

// 周期跳动的 now 值：运行时长计时、相对时间刷新共用。
export function useTicker(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [intervalMs]);

  return now;
}
