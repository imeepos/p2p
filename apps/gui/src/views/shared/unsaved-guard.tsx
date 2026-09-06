import { useEffect, useRef, useState, type ReactNode } from "react";
import { useBlocker, useLocation, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import {
  discardAllUnsaved,
  hasAnyUnsaved,
} from "@/views/shared/use-unsaved-guard";
import { historyIdxOf, setPopStateHandler } from "./pop-nav-guard";

type RouteLocation = { pathname: string; search: string; hash: string };

function sameLocation(a: RouteLocation, b: RouteLocation): boolean {
  return a.pathname === b.pathname && a.search === b.search && a.hash === b.hash;
}

// 回滚锚点：最近一次 router 落定的文档 URL 与 history.state（含 idx），
// delta==null 的 POP 兜底用它恢复地址与状态。
interface SafeAnchor {
  href: string;
  state: unknown;
}

// 路由守卫组件：包裹受保护路由的 element。任一注册的编辑面脏状态时，
// 任何导航（侧栏点击、快捷键、浏览器返回）先弹确认——确认放弃则丢弃
// 草稿放行，取消则驻留当前页。POP 直改 hash 路径由 pop-nav-guard 兜底。
export function UnsavedRouteGuard({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const navigate = useNavigate();
  const location = useLocation();
  // 同址 POP（兜底回滚后的残留事件）不拦，交 router 原地消化，避免双弹窗
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    hasAnyUnsaved() && !sameLocation(currentLocation, nextLocation),
  );
  const [popTarget, setPopTarget] = useState<string | null>(null);
  // confirm 回调异步触发，晚于本渲染：ref 在 effect 中同步（render 期禁写），
  // 回调时取到的 proceed/reset 与 router 当前 pending 导航一致。
  const blockerRef = useRef(blocker);
  const safeRef = useRef<SafeAnchor>({ href: "", state: null });
  const state = blocker.state;

  useEffect(() => {
    blockerRef.current = blocker;
  });

  useEffect(() => {
    safeRef.current = {
      href: "#" + location.pathname + location.search + location.hash,
      state: window.history.state,
    };
  }, [location]);

  // F05 兜底：脏且 delta 不可判（idx 缺失）的 POP，router 会静默放行——
  // 先回滚 URL 再弹确认；delta 可判的返回交由 useBlocker，不双拦。
  useEffect(() => {
    setPopStateHandler((event) => {
      if (!hasAnyUnsaved()) return;
      const targetHref = window.location.hash;
      if (targetHref === safeRef.current.href) return;
      const leavingKnown = historyIdxOf(safeRef.current.state) !== null;
      const targetKnown = historyIdxOf(event.state) !== null;
      if (leavingKnown && targetKnown) return;
      window.history.replaceState(
        safeRef.current.state,
        "",
        safeRef.current.href,
      );
      setPopTarget(targetHref);
    });
    return () => setPopStateHandler(null);
  }, []);

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

  // 兜底确认：放弃则丢草稿并 replace 到目标路由；取消则驻留（URL 已回滚）。
  // 依赖只挂 popTarget：confirm/t/navigate 经 ref 取最新值，i18n t 身份
  // 变化不得重启本 effect（重启会换弹窗实例，点击落在已卸载节点上失效）。
  const dialogDepsRef = useRef({ confirm, t, navigate });
  useEffect(() => {
    dialogDepsRef.current = { confirm, t, navigate };
  });
  useEffect(() => {
    if (popTarget === null) return;
    let active = true;
    const { confirm, t, navigate } = dialogDepsRef.current;
    void confirm({
      title: t("settings.unsavedGuard.title"),
      description: t("settings.unsavedGuard.description"),
      confirmText: t("settings.unsavedGuard.discard"),
      cancelText: t("settings.unsavedGuard.stay"),
      destructive: true,
    }).then((leave) => {
      if (!active) return;
      setPopTarget(null);
      if (leave) {
        discardAllUnsaved();
        const route = popTarget.startsWith("#") ? popTarget.slice(1) : popTarget;
        navigate(route || "/", { replace: true });
      }
    });
    return () => {
      active = false;
    };
  }, [popTarget]);

  return children;
}
