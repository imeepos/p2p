import type { Locale } from "@/i18n";
import { formatTimeShort } from "@/lib/format";

export function TimeDivider({ tsMs, locale }: { tsMs: number; locale: Locale }) {
  return (
    <div className="my-1 flex justify-center" role="separator">
      <time className="text-muted-foreground/80 px-2 text-xs select-none">
        {formatTimeShort(tsMs, locale)}
      </time>
    </div>
  );
}
