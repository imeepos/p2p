import { useId, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

export type SparklineTone = "success" | "warning";

interface SparklinePoint {
  tMs: number;
  value: number;
}

interface SparklineProps {
  points: SparklinePoint[];
  tone?: SparklineTone;
  label: string;
  formatValue?: (value: number) => string;
  className?: string;
}

const TONE_STROKE: Record<SparklineTone, string> = {
  success: "stroke-success",
  warning: "stroke-warning",
};

const TONE_BADGE: Record<SparklineTone, string> = {
  success: "border-success/40 bg-success/10 text-success",
  warning: "border-warning/40 bg-warning/10 text-warning",
};

function buildPath(values: number[], width: number, height: number): string {
  if (values.length === 0) return "";
  if (values.length === 1) return `M0 ${height / 2} L${width} ${height / 2}`;
  const max = Math.max(...values);
  const min = Math.min(...values);
  const span = max - min || 1;
  const stepX = width / (values.length - 1);
  return values
    .map((value, index) => {
      const x = index * stepX;
      const y = height - ((value - min) / span) * (height - 4) - 2;
      return `${index === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(" ");
}

function lastPointXY(values: number[], width: number, height: number): { x: number; y: number } {
  const max = Math.max(...values);
  const min = Math.min(...values);
  const span = max - min || 1;
  return {
    x: width,
    y: height - ((values[values.length - 1] - min) / span) * (height - 4) - 2,
  };
}

const VIEW_W = 240;
const VIEW_H = 48;

// 纯 SVG sparkline：归一化折线 + 末点徽标 + hover tooltip；reduced-motion 禁过渡。
export function Sparkline({
  points,
  tone = "success",
  label,
  formatValue,
  className,
}: SparklineProps) {
  const { t } = useTranslation();
  const gradientId = useId();
  const [hover, setHover] = useState<SparklinePoint | null>(null);
  const values = useMemo(() => points.map((point) => point.value), [points]);
  const path = useMemo(() => buildPath(values, VIEW_W, VIEW_H), [values]);
  const end = values.length > 0 ? lastPointXY(values, VIEW_W, VIEW_H) : null;
  const last = points[points.length - 1] ?? null;
  const shown = hover ?? last;
  const format = formatValue ?? ((value: number) => String(value));

  return (
    <div className={cn("relative", className)}>
      <svg
        viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
        preserveAspectRatio="none"
        className="h-12 w-full"
        role="img"
        aria-label={label}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(event) => {
          if (points.length === 0) return;
          const rect = event.currentTarget.getBoundingClientRect();
          const ratio = (event.clientX - rect.left) / rect.width;
          const index = Math.min(
            points.length - 1,
            Math.max(0, Math.round(ratio * (points.length - 1))),
          );
          setHover(points[index]);
        }}
      >
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="currentColor" stopOpacity="0.25" />
            <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
          </linearGradient>
        </defs>
        {path && (
          <>
            <path
              d={`${path} L${VIEW_W} ${VIEW_H} L0 ${VIEW_H} Z`}
              fill={`url(#${gradientId})`}
              className={TONE_STROKE[tone]}
              stroke="none"
            />
            <path
              d={path}
              fill="none"
              strokeWidth="1.5"
              className={cn(TONE_STROKE[tone], "motion-safe:transition-all")}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          </>
        )}
        {end && (
          <circle
            cx={end.x - 2}
            cy={end.y}
            r="2.5"
            className={cn(TONE_STROKE[tone], "fill-background")}
            strokeWidth="1.5"
          />
        )}
      </svg>
      {shown && (
        <span className="text-muted-foreground pointer-events-none absolute top-0 right-0 font-mono text-[10px] tabular-nums">
          {new Intl.DateTimeFormat(undefined, { timeStyle: "medium" }).format(shown.tMs)}
        </span>
      )}
      {last && (
        <span
          className={cn(
            "absolute top-0 left-0 rounded border px-1.5 py-0.5 font-mono text-xs font-medium tabular-nums",
            TONE_BADGE[tone],
          )}
        >
          {t("dashboard.trend.now", { count: format(last.value) })}
        </span>
      )}
    </div>
  );
}
