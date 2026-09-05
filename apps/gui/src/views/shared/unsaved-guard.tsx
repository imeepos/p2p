import { useEffect, useRef, type ReactNode } from "react";
import { useBlocker } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";

// 脏状态守卫（路由层）：设置表单 / 节点资料草稿 / 中继地址列表 / 发现页
// mDNS 开关草稿在各自视图内注册，离开路由（侧栏点击或快捷键 navigate）
// 统一在 useBlocker 拦截，两条导航路径天然同权。
interface UnsavedGuardHandler {
  hasUnsaved: () => boolean;
  discard: () => void;
}

interface RegisteredGuard {
  key: string;
  handler: UnsavedGuardHandler;
}

const guards = new Map<string, RegisteredGuard>();

export function registerUnsavedGuard(
  key: string,
  handler: UnsavedGuardHandler,
): () => void {
  guards.set(key, { key, handler });
  return () => {
    const current = guards.get(key);
    if (current?.handler === handler) guards.delete(key);
  };
}

export function hasAnyUnsaved(): boolean {
  for (const guard of guards.values()) {
    if (guard.handler.hasUnsaved()) return true;
  }
  return false;
}

// 放弃离开：各编辑面丢弃本地草稿（导航前同步执行，卸载后置 state 无副作用）
export function discardAllUnsaved(): void {
  for (const guard of guards.values()) guard.handler.discard();
}

// 视图内注册：handler 闭包随渲染更新（ref 中转），卸载自动注销。
export function useUnsavedGuard(key: string, handler: UnsavedGuardHandler): void {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(
    () =>
      registerUnsavedGuard(key, {
        hasUnsaved: () => latest.current.hasUnsaved(),
        discard: () => latest.current.discard(),
      }),
    [key],
  );
}

// 路由守卫组件：包裹受保护路由的 element。脏状态时任何导航先弹确认，
// 确认放弃则丢弃草稿放行，取消则驻留当前页。
export function UnsavedRouteGuard({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const blocker = useBlocker(() => hasAnyUnsaved());
  const blockerRef = useRef(blocker);
  blockerRef.current = blocker;
  const state = blocker.state;

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
