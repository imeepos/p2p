import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import { cn } from "@/lib/utils";

// 轻量右键菜单原语：固定定位 + 视口内钳制，外点/Esc/滚动/失焦关闭。
// 仓库未引入 radix context-menu 依赖，自绘保持零新增依赖；键盘保证
// Esc 关闭与上下键移动焦点（菜单项为普通 button，天然可聚焦）。

export interface ContextMenuAnchor {
  x: number;
  y: number;
}

interface ContextMenuProps {
  anchor: ContextMenuAnchor | null;
  /** aria-label（角色 menu 的可达名） */
  label: string;
  onClose: () => void;
  children: ReactNode;
  testId?: string;
}

const CloseContext = createContext<() => void>(() => {});

const VIEWPORT_MARGIN = 8;

export function ContextMenu({ anchor, label, onClose, children, testId }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState<{ x: number; y: number; clamped: boolean } | null>(null);
  const [renderedAnchor, setRenderedAnchor] = useState<ContextMenuAnchor | null>(null);

  // anchor 变化在渲染期复位（React 官方 props 调整模式）：先落原始锚点并隐藏，
  // 随后 rAF 回调里量尺寸钳制进视口——setState 不在 effect 体同步调用。
  if (anchor !== renderedAnchor) {
    setRenderedAnchor(anchor);
    setPos(anchor ? { x: anchor.x, y: anchor.y, clamped: false } : null);
  }

  useEffect(() => {
    if (!anchor) return;
    const el = ref.current;
    const frame = requestAnimationFrame(() => {
      if (!el) return;
      const rect = el.getBoundingClientRect();
      setPos({
        x: Math.max(VIEWPORT_MARGIN, Math.min(anchor.x, window.innerWidth - rect.width - VIEWPORT_MARGIN)),
        y: Math.max(VIEWPORT_MARGIN, Math.min(anchor.y, window.innerHeight - rect.height - VIEWPORT_MARGIN)),
        clamped: true,
      });
    });
    return () => cancelAnimationFrame(frame);
  }, [anchor]);

  useEffect(() => {
    if (!anchor) return;
    const outside = (target: EventTarget | null): boolean =>
      ref.current !== null && target instanceof Node && ref.current.contains(target);
    const onPointerDown = (event: PointerEvent) => {
      if (!outside(event.target)) onClose();
    };
    const onContextMenu = (event: MouseEvent) => {
      if (!outside(event.target)) onClose();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    // 打开即聚焦容器，键盘用户无需再点一次
    ref.current?.focus();
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("contextmenu", onContextMenu);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("resize", onClose);
    window.addEventListener("scroll", onClose, true);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("contextmenu", onContextMenu);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("scroll", onClose, true);
      window.removeEventListener("blur", onClose);
    };
  }, [anchor, onClose]);

  const focusItem = (dir: 1 | -1) => {
    const items = Array.from(
      ref.current?.querySelectorAll<HTMLButtonElement>("button[role='menuitem']:not(:disabled)") ?? [],
    );
    if (items.length === 0) return;
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = items[(index + dir + items.length) % items.length];
    next?.focus();
  };

  const onKeyDown = (event: ReactKeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusItem(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusItem(-1);
    }
  };

  if (!anchor) return null;
  return createPortal(
    <CloseContext.Provider value={onClose}>
      <div
        ref={ref}
        role="menu"
        aria-label={label}
        tabIndex={-1}
        data-testid={testId}
        onKeyDown={onKeyDown}
        style={{
          position: "fixed",
          left: pos?.x ?? 0,
          top: pos?.y ?? 0,
          visibility: pos?.clamped ? "visible" : "hidden",
        }}
        className="bg-popover text-popover-foreground border-border/60 shadow-md z-50 min-w-40 rounded-md border p-1 outline-none"
      >
        <ul className="flex flex-col">{children}</ul>
      </div>
    </CloseContext.Provider>,
    document.body,
  );
}

export interface ContextMenuItemProps {
  onSelect: () => void;
  children: ReactNode;
  destructive?: boolean;
  disabled?: boolean;
  testId?: string;
}

export function ContextMenuItem({ onSelect, children, destructive, disabled, testId }: ContextMenuItemProps) {
  const close = useContext(CloseContext);
  return (
    <li role="none">
      <button
        type="button"
        role="menuitem"
        disabled={disabled}
        data-testid={testId}
        onClick={() => {
          onSelect();
          close();
        }}
        className={cn(
          "focus:bg-wx-hover hover:bg-wx-hover w-full rounded-sm px-3 py-1.5 text-left text-[13px] outline-none disabled:cursor-not-allowed disabled:opacity-50",
          destructive && "text-destructive focus:bg-destructive/10 hover:bg-destructive/10",
        )}
      >
        {children}
      </button>
    </li>
  );
}

export function ContextMenuSeparator() {
  return <li aria-hidden role="separator" className="bg-border mx-2 my-1 h-px" />;
}
