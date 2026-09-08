import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

// 行/资料卡共用的圆角方头像（微信式）：有 src（对端自报头像 data URL）
// 渲染图片，否则回退首字；icon 覆盖首字（Agent 用）。
export function ContactAvatar(props: {
  initial: string;
  src?: string | null;
  className?: string;
  icon?: ReactNode;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        "bg-primary/10 text-primary flex shrink-0 items-center justify-center overflow-hidden rounded-md font-semibold",
        props.className ?? "size-9 text-sm",
      )}
    >
      {props.src ? (
        <img src={props.src} alt="" className="size-full object-cover" />
      ) : (
        (props.icon ?? props.initial)
      )}
    </span>
  );
}

// 行容器与行内动作区的共享类：动作悬停显隐，静止时保持微信式干净行。
export const CONTACT_ROW_CLS =
  "group flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left";
export const ROW_ACTIONS_CLS =
  "flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100";
