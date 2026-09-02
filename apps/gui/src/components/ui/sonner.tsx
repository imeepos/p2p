import type { CSSProperties } from "react";

import { Toaster as Sonner, type ToasterProps } from "sonner";

import { useTheme } from "@/theme/theme-provider";

const AppToaster = ({ ...props }: ToasterProps) => {
  const { resolvedTheme } = useTheme();

  return (
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
  );
};

export { AppToaster };
