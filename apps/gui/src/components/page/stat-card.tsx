import type { ReactNode } from "react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

export type CardSpan = 3 | 4 | 6 | 12;

const SPAN_CLASS: Record<CardSpan, string> = {
  3: "col-span-12 sm:col-span-6 lg:col-span-3",
  4: "col-span-12 sm:col-span-6 lg:col-span-4",
  6: "col-span-12 lg:col-span-6",
  12: "col-span-12",
};

interface StatCardProps {
  label: string;
  value?: ReactNode;
  span?: CardSpan;
  loading?: boolean;
  mono?: boolean;
}

export function StatCard({
  label,
  value,
  span = 3,
  loading = false,
  mono = false,
}: StatCardProps) {
  return (
    <div className={SPAN_CLASS[span]}>
      <Card className="gap-3 py-4">
        <CardHeader className="px-4">
          <CardDescription>{label}</CardDescription>
          <CardTitle
            className={cn("text-lg", mono && "font-mono text-sm break-all")}
          >
            {loading ? <Skeleton className="h-5 w-28" /> : (value ?? "-")}
          </CardTitle>
        </CardHeader>
        <CardContent className="px-4" />
      </Card>
    </div>
  );
}
