import { useCallback, useRef } from "react";

// IME 组合态判定：输入法选词期间的 Enter 是「确认候选词」，不得触发发送。
// isComposing 覆盖标准路径；keyCode 229 兜底个别浏览器/驱动不置 isComposing
// 的组合态（keydown 在 IME 处理中统一上报 229）。语义与 acp/ime-guard.ts
// 一致；本钩子为 chat 域等共享入口，acp 收敛迁移由协调者统一处理。
export interface KeyboardEventLike {
  isComposing?: boolean | undefined;
  keyCode?: number | undefined;
}

export function isImeComposing(event: KeyboardEventLike): boolean {
  return event.isComposing === true || event.keyCode === 229;
}

export interface ImeCompositionGuard {
  /** 挂到输入元素：追踪组合事件进行中（compositionstart 至 compositionend）。 */
  compositionHandlers: {
    onCompositionStart: () => void;
    onCompositionEnd: () => void;
  };
  /** 组合事件进行中或组合键码 → 该 Enter 应被拦截（不发送、不换行）。 */
  shouldBlockEnter: (event: KeyboardEventLike) => boolean;
}

export function useImeCompositionGuard(): ImeCompositionGuard {
  const composingRef = useRef(false);
  const onCompositionStart = useCallback(() => {
    composingRef.current = true;
  }, []);
  const onCompositionEnd = useCallback(() => {
    composingRef.current = false;
  }, []);
  const shouldBlockEnter = useCallback(
    (event: KeyboardEventLike) => composingRef.current || isImeComposing(event),
    [],
  );
  return {
    compositionHandlers: { onCompositionStart, onCompositionEnd },
    shouldBlockEnter,
  };
}
