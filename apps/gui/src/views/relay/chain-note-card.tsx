import { MoveRightIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const STEPS = ["direct", "punch", "relay"] as const;

// 降级链说明卡：直连 -> 打洞 -> 中继，以及 relay 在链路里的兜底角色。
export function ChainNoteCard() {
  const { t } = useTranslation();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.chain.title")}</CardTitle>
        <CardDescription>{t("relay.chain.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          {STEPS.map((step, index) => (
            <span key={step} className="flex items-center gap-2">
              {index > 0 ? (
                <MoveRightIcon
                  className="text-muted-foreground size-4"
                  aria-hidden
                />
              ) : null}
              <span className="rounded-md border px-2 py-0.5 text-xs">
                {t(`relay.chain.${step}`)}
              </span>
            </span>
          ))}
        </div>
        <p className="text-sm">{t("relay.chain.desc")}</p>
        <p className="text-muted-foreground text-xs">
          {t("relay.chain.relayRole")}
        </p>
      </CardContent>
    </Card>
  );
}
