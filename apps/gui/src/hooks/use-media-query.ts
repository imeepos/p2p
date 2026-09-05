import { useEffect, useState } from "react";

// 响应式断点钩子：jsdom 无布局，测试经 matchMedia 桩驱动（§2.1 <768 单栏互斥）。
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);
  return matches;
}

/** §2.1 窄屏断点：<768 单栏互斥（桌面壳防御性规则） */
export const NARROW_CHAT_QUERY = "(max-width: 767.98px)";