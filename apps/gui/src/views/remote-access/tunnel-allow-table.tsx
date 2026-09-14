import { useTranslation } from "react-i18next";
import type { Locale } from "@/i18n";

import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatDateTime } from "@/lib/format";

// serve 白名单表格化（gap-matrix §4 纯前端栏 S1）：端口/加入时间/状态。
// 加入时间为本会话内经 GUI 开启的记录（契约 serve.allow 无时间字段），
// 快照遗留条目如实显示「—」，不虚构时间。
export function TunnelAllowTable({
  allow,
  enabled,
  addedAtByTarget,
  locale,
}: {
  allow: string[];
  enabled: boolean;
  addedAtByTarget: Record<string, number | null>;
  locale: Locale;
}) {
  const { t } = useTranslation();
  if (allow.length === 0) {
    return <p className="text-muted-foreground text-sm">{t("remoteAccess.serve.emptyAllow")}</p>;
  }
  return (
    <Table data-testid="tunnel-allow-table">
      <TableHeader>
        <TableRow>
          <TableHead>{t("remoteAccess.tunnel.allowTablePort")}</TableHead>
          <TableHead>{t("remoteAccess.tunnel.allowTableAddedAt")}</TableHead>
          <TableHead>{t("remoteAccess.tunnel.allowTableStatus")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {allow.map((target) => (
          <TableRow key={target} data-testid="tunnel-allow-row">
            <TableCell className="font-mono text-xs">{target}</TableCell>
            <TableCell className="text-xs">
              {addedAtByTarget[target]
                ? formatDateTime(addedAtByTarget[target] as number, locale)
                : "—"}
            </TableCell>
            <TableCell>
              <Badge variant={enabled ? "default" : "secondary"}>
                {enabled
                  ? t("remoteAccess.serve.statusEnabled")
                  : t("remoteAccess.serve.statusDisabled")}
              </Badge>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
