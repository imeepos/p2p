import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// 资料卡共享骨架（微信联系人详情式）：头部（大头像 + 标题行）+ 分行字段 +
// 底部居中动作区（图标在上文案在下）。
export function DetailShell(props: { avatar: ReactNode; title: ReactNode; children: ReactNode }) {
  return (
    <div className="mx-auto flex w-full max-w-md flex-col px-6 py-8">
      <div className="flex items-start gap-4">
        {props.avatar}
        <div className="min-w-0 flex-1 pt-1">{props.title}</div>
      </div>
      {props.children}
    </div>
  );
}

export function DetailRows(props: { children: ReactNode }) {
  return <div className="mt-4 flex flex-col divide-y border-t">{props.children}</div>;
}

export function DetailRow(props: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-start gap-6 py-2.5 text-sm">
      <span className="text-muted-foreground w-20 shrink-0">{props.label}</span>
      <span className="flex min-w-0 flex-1 items-center gap-1">{props.children}</span>
    </div>
  );
}

export function DetailActions(props: { children: ReactNode }) {
  return (
    <div className="mt-2 flex items-start justify-center gap-10 border-t pt-6">{props.children}</div>
  );
}

const ACTION_CLS = "h-auto flex-col gap-1.5 px-3 py-2";

// 底部动作钮（图标在上、文案在下）；destructive 用于删除/退群等危险动作。
export function DetailAction(props: {
  icon: LucideIcon;
  label: string;
  testId?: string;
  disabled?: boolean;
  destructive?: boolean;
  title?: string;
  onClick?: () => void;
}) {
  const Icon = props.icon;
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      className={cn(ACTION_CLS, props.destructive && "text-destructive hover:text-destructive")}
      data-testid={props.testId}
      disabled={props.disabled}
      title={props.title}
      onClick={props.onClick}
    >
      <Icon aria-hidden className="size-5" />
      <span className="text-xs font-normal">{props.label}</span>
    </Button>
  );
}

export function DetailActionLink(props: {
  icon: LucideIcon;
  label: string;
  testId: string;
  to: string;
}) {
  const Icon = props.icon;
  return (
    <Button type="button" variant="ghost" size="sm" className={ACTION_CLS} asChild>
      <Link to={props.to} data-testid={props.testId}>
        <Icon aria-hidden className="size-5" />
        <span className="text-xs font-normal">{props.label}</span>
      </Link>
    </Button>
  );
}
