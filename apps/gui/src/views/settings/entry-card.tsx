import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { I18nKey } from "@/i18n/types";
import { SettingsGroup, SettingsRow } from "./settings-row";

export interface EntryCardBadge {
  count: number;
  ariaKey: I18nKey;
}

interface EntryCardProps {
  path: string;
  titleKey: I18nKey;
  descKey: I18nKey;
  icon: LucideIcon;
  badge?: EntryCardBadge;
  testId?: string;
}

// 设置页入口行卡（2026-09-18 IA 重组的通用件）：标题/描述复用目标页既有
// i18n 键，动作键统一 settings.entryAction。运维区与远程访问业务页入口共用。
export function EntryCard({
  path,
  titleKey,
  descKey,
  icon: Icon,
  badge,
  testId,
}: EntryCardProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <SettingsGroup title={t(titleKey)} description={t(descKey)}>
      <SettingsRow
        control={
          <div className="flex items-center gap-2">
            {badge != null && badge.count > 0 ? (
              <Badge
                variant="destructive"
                aria-label={t(badge.ariaKey, { count: badge.count })}
                data-testid={`entry-badge-${path}`}
              >
                {badge.count}
              </Badge>
            ) : null}
            <Icon aria-hidden className="text-muted-foreground size-4" />
            <Button
              type="button"
              size="sm"
              variant="outline"
              data-testid={testId}
              onClick={() => navigate(path)}
            >
              {t("settings.entryAction")}
            </Button>
          </div>
        }
      />
    </SettingsGroup>
  );
}
