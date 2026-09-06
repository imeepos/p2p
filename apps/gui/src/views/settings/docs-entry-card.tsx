import { BookOpen } from "lucide-react";
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

// 设置页协议文档入口行（DOC2）：只读跳转 /docs；不进 rail（rail 保持 4 项）。
export function DocsEntryCard() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <BookOpen className="size-4" aria-hidden />
          {t("docs.settings.entry")}
        </CardTitle>
        <CardDescription>{t("docs.settings.entryDescription")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => navigate("/docs")}
        >
          {t("docs.settings.entryAction")}
        </Button>
      </CardContent>
    </Card>
  );
}
