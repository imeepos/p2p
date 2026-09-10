import { useEffect, useLayoutEffect, useRef } from "react";

import type { ChatMessageJson } from "@/lib/ipc-types";

// 普通渲染路径（短列表）的滚动行为钩子：钉底跟随 + 前插补偿。
// 虚拟路径不用这些（firstItemIndex 锚定 + atBottom 跟随在虚拟流内实现）。

// 前插滚动补偿（UX5）：WebKit 无滚动锚定，前插更早历史后视口内容整体跳位。
// 以「首条消息 id 变化且旧首条仍在列表」识别前插，在布局提交阶段把
// scrollTop 平移高度增量，视口锚定不跳。
export function usePrependScrollCompensation(
  scrollRef: React.RefObject<HTMLDivElement | null>,
  messages: ChatMessageJson[],
  stickBottomRef: React.MutableRefObject<boolean>,
): void {
  const lastFirstIdRef = useRef<string | null>(null);
  const lastScrollHeightRef = useRef(0);
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const firstId = messages[0]?.id ?? null;
    const prevFirstId = lastFirstIdRef.current;
    const prepended =
      prevFirstId !== null &&
      firstId !== prevFirstId &&
      messages.some((m) => m.id === prevFirstId);
    if (prepended && !stickBottomRef.current) {
      const delta = el.scrollHeight - lastScrollHeightRef.current;
      if (delta > 0) el.scrollTop += delta;
    }
    lastFirstIdRef.current = firstId;
    lastScrollHeightRef.current = el.scrollHeight;
  }, [messages, scrollRef, stickBottomRef]);
}

// 钉底跟随：切换会话强制钉底；新消息到达且钉底态时回到底部。
export function useStickToBottom(
  scrollRef: React.RefObject<HTMLDivElement | null>,
  messages: ChatMessageJson[],
  peer: string,
): React.MutableRefObject<boolean> {
  const stickBottomRef = useRef(true);
  useEffect(() => {
    stickBottomRef.current = true;
  }, [peer]);
  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current) el.scrollTop = el.scrollHeight;
  }, [messages, peer, scrollRef]);
  return stickBottomRef;
}
