import { useEffect, useRef, type ReactNode } from "react";
import { useBlocker } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import {
  discardAllUnsaved,
  hasAnyUnsaved,
} from "@/views/shared/use-unsaved-guard";

// 路由守卫组件：包裹受保护路由的 element。任一注册的编辑面脏状态时，
// 任何导航（侧栏点击、快捷键、浏览器返回）先弹确认——确认放弃则丢弃
// 草稿放行，取消则驻留当前页。
export function UnsavedRouteGuard({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const blocker = useBlocker(() => hasAnyUnsaved());
  // confirm 回调异步触发，晚于本渲染：ref 在 effect 中同步（render 期禁写），
  // 回调时取到的 proceed/reset 与 router 当前 pending 导航一致。
  const blockerRef = useRef(blocker);
  const state = blocker.state;

  useEffect(() => {
    blockerRef.current = blocker;
  });

  useEffect(() => {
    if (state !== "blocked") return;
    let active = true;
    void confirm({
      title: t("settings.unsavedGuard.title"),
      description: t("settings.unsavedGuard.description"),
      confirmText: t("settings.unsavedGuard.discard"),
      cancelText: t("settings.unsavedGuard.stay"),
      destructive: true,
    }).then((leave) => {
      if (!active) return;
      if (leave) {
        discardAllUnsaved();
        blockerRef.current.proceed?.();
      } else {
        blockerRef.current.reset?.();
      }
    });
    return () => {
      active = false;
    };
  }, [state, confirm, t]);

  return children;
}
