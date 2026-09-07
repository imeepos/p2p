import { useTranslation } from "react-i18next";

import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useUiPrefsStore } from "@/stores/ui-prefs-store";

// 已退出/已解散群聊开关（默认隐藏）：本端存在非 active 群或开关已开时才
// 渲染，无相关群不占侧栏空间；偏好写穿 localStorage（ui-prefs-store）。
export function InactiveGroupsToggle({ hiddenCount }: { hiddenCount: number }) {
  const { t } = useTranslation();
  const show = useUiPrefsStore((s) => s.showInactiveGroups);
  const setShow = useUiPrefsStore((s) => s.setShowInactiveGroups);
  if (hiddenCount === 0 && !show) return null;
  return (
    <div
      data-testid="inactive-groups-toggle"
      className="border-border/60 flex items-center gap-2 border-t px-3 py-2"
    >
      <Switch
        id="inactive-groups-switch"
        checked={show}
        onCheckedChange={setShow}
        aria-label={t("chat.conversations.inactiveToggle")}
        data-testid="inactive-groups-switch"
      />
      <Label
        htmlFor="inactive-groups-switch"
        className="text-muted-foreground text-xs font-normal"
      >
        {t("chat.conversations.inactiveToggle", { count: hiddenCount })}
      </Label>
    </div>
  );
}
