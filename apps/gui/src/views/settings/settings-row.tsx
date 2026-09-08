import type { ReactNode } from "react";

import { Label } from "@/components/ui/label";

interface SettingsRowProps {
  htmlFor?: string;
  label?: string;
  description?: ReactNode;
  error?: ReactNode;
  control?: ReactNode;
}

// 微信设置式单行：左标签 + 灰说明（校验错误也挂左列），右侧控件对齐。
// 行间不加分隔线，节奏靠 py-3；组间距由 SettingsGroup 承担。
export function SettingsRow({
  htmlFor,
  label,
  description,
  error,
  control,
}: SettingsRowProps) {
  return (
    <div className="flex items-center justify-between gap-6 py-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        {label != null ? <Label htmlFor={htmlFor}>{label}</Label> : null}
        {description != null ? (
          <div className="text-muted-foreground text-xs leading-5">
            {description}
          </div>
        ) : null}
        {error}
      </div>
      {control != null ? (
        <div className="shrink-0">{control}</div>
      ) : null}
    </div>
  );
}

interface SettingsGroupProps {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}

// 分组：小号加粗组头 + 行列表，对应参考稿的「账号 / 存储」分节节奏。
export function SettingsGroup({
  title,
  description,
  children,
}: SettingsGroupProps) {
  return (
    <section className="flex flex-col">
      <h2 className="text-sm font-semibold">{title}</h2>
      {description != null ? (
        <p className="text-muted-foreground mt-0.5 text-xs leading-5">
          {description}
        </p>
      ) : null}
      <div className="mt-1 flex flex-col">{children}</div>
    </section>
  );
}

interface SettingsBlockProps {
  children: ReactNode;
}

// 整行块：地址列表编辑器这类自带标签的多行内容，不套左右分栏。
export function SettingsBlock({ children }: SettingsBlockProps) {
  return <div className="flex flex-col py-3">{children}</div>;
}
