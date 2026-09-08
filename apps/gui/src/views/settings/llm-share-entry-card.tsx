import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { SettingsGroup, SettingsRow } from "./settings-row";

// 设置页 llm-share 入口卡（契约 v11 §16.3）：一句话说明 + 跳转 /llm-share；
// 页面本体归 LSG3（四面板），入口先行落位（/docs 先例：命令面板+设置页可达，不进 rail）。
export function LlmShareEntryCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <SettingsGroup
      title={t("settings.llmShare.entry")}
      description={t("settings.llmShare.entryDescription")}
    >
      <SettingsRow
        control={
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => navigate("/llm-share")}
          >
            {t("settings.llmShare.entryAction")}
          </Button>
        }
      />
    </SettingsGroup>
  );
}
