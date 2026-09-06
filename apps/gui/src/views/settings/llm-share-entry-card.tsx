import { Share2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

// 设置页 llm-share 入口卡（契约 v11 §16.3）：一句话说明 + 跳转 /llm-share；
// 页面本体归 LSG3（四面板），入口先行落位（/docs 先例：命令面板+设置页可达，不进 rail）。
export function LlmShareEntryCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Share2 className="size-4" aria-hidden />
          {t("settings.llmShare.entry")}
        </CardTitle>
        <CardDescription>{t("settings.llmShare.entryDescription")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => navigate("/llm-share")}
        >
          {t("settings.llmShare.entryAction")}
        </Button>
      </CardContent>
    </Card>
  );
}
