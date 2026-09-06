import type { CSSProperties, KeyboardEvent, MouseEvent } from "react";

import { toast, Toaster as Sonner, type ToasterProps } from "sonner";

import { useTheme } from "@/theme/theme-provider";

// sonner v2 移除了 closeOnClick：toast 根节点无 onClick，点击不关闭。
// 这里在容器上做事件委托：点 toast 本体关单条（testid 即 toast id，toast.ts）；
// 点按钮/链接、正选中文本（复制意图）时不关。
function resolveToastId(target: EventTarget | null): string | null {
  const el = target as HTMLElement | null;
  if (!el || typeof el.closest !== "function") return null;
  if (el.closest("button, a, input, textarea, [data-button]")) return null;
  const li = el.closest<HTMLElement>("[data-sonner-toast]");
  return li?.dataset.testid ?? null;
}

function handleToasterClick(event: MouseEvent<HTMLDivElement>): void {
  const id = resolveToastId(event.target);
  if (!id) return;
  if ((window.getSelection()?.toString().length ?? 0) > 0) return;
  toast.dismiss(id);
}

function handleToasterKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
  if (event.key !== "Enter" && event.key !== " ") return;
  const id = resolveToastId(event.target);
  if (!id) return;
  event.preventDefault();
  toast.dismiss(id);
}

const AppToaster = ({ ...props }: ToasterProps) => {
  const { resolvedTheme } = useTheme();

  return (
    <div onClick={handleToasterClick} onKeyDown={handleToasterKeyDown}>
      <Sonner
        theme={resolvedTheme}
        className="toaster group"
        style={
          {
            "--normal-bg": "var(--popover)",
            "--normal-text": "var(--popover-foreground)",
            "--normal-border": "var(--border)",
          } as CSSProperties
        }
        {...props}
      />
    </div>
  );
};

export { AppToaster };
