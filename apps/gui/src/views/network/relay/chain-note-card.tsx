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
// 步骤条形态（IM-V1 R1）：数字徽章 + 箭头，统一间距、垂直居中。
export function ChainNoteCard() {
  const { t } = useTranslation();

  return (
    <Card className="col-span-12 h-full lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.chain.title")}</CardTitle>
        <CardDescription>{t("relay.chain.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <ol className="flex flex-wrap items-center gap-2">
          {STEPS.map((step, index) => (
            <li key={step} className="flex items-center gap-2">
              {index > 0 ? (
                <MoveRightIcon
                  className="text-muted-foreground size-4"
                  aria-hidden
                />
              ) : null}
              <span className="border bg-muted/50 flex items-center gap-1.5 rounded-md px-2 py-1 text-xs">
                <span
                  aria-hidden
                  className="bg-primary/10 text-primary flex size-5 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold tabular-nums"
                >
                  {index + 1}
                </span>
                {t(`relay.chain.${step}`)}
              </span>
            </li>
          ))}
        </ol>
        <p className="text-sm">{t("relay.chain.desc")}</p>
        <p className="text-muted-foreground text-xs">
          {t("relay.chain.relayRole")}
        </p>
      </CardContent>
    </Card>
  );
}
