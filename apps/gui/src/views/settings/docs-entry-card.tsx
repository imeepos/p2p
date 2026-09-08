import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { SettingsGroup, SettingsRow } from "./settings-row";

// 设置页协议文档入口行（DOC2）：只读跳转 /docs；不进 rail（rail 保持 4 项）。
export function DocsEntryCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <SettingsGroup
      title={t("docs.settings.entry")}
      description={t("docs.settings.entryDescription")}
    >
      <SettingsRow
        control={
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => navigate("/docs")}
          >
            {t("docs.settings.entryAction")}
          </Button>
        }
      />
    </SettingsGroup>
  );
}
