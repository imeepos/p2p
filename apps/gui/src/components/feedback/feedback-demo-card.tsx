import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

const SIMULATED_DELAY_MS = 900;

export function FeedbackDemoCard() {
  const { t } = useTranslation();
  const confirm = useConfirm();

  const simulateAsync = () =>
    new Promise((resolve) => window.setTimeout(resolve, SIMULATED_DELAY_MS));

  const runConfirm = async () => {
    const ok = await confirm({
      title: t("dashboard.demo.confirmTitle"),
      description: t("dashboard.demo.confirmDescription"),
      confirmText: t("common.actions.confirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (ok) {
      toastSuccess(t("dashboard.demo.confirmed"));
    } else {
      toastError(t("dashboard.demo.cancelled"));
    }
  };

  const rows: { label: string; hint: string; node: React.ReactNode }[] = [
    {
      label: t("dashboard.demo.async"),
      hint: t("dashboard.demo.asyncHint"),
      node: (
        <AsyncButton
          size="sm"
          action={simulateAsync}
          onSuccess={() => toastSuccess(t("dashboard.demo.asyncDone"))}
          onError={(error) => toastError(String(error))}
        >
          {t("dashboard.demo.async")}
        </AsyncButton>
      ),
    },
    {
      label: t("dashboard.demo.toastSuccess"),
      hint: t("dashboard.demo.toastSuccessHint"),
      node: (
        <Button
          size="sm"
          variant="outline"
          onClick={() => toastSuccess(t("dashboard.demo.toastSuccess"))}
        >
          {t("dashboard.demo.toastSuccess")}
        </Button>
      ),
    },
    {
      label: t("dashboard.demo.toastError"),
      hint: t("dashboard.demo.toastErrorHint"),
      node: (
        <Button
          size="sm"
          variant="outline"
          onClick={() => toastError(t("dashboard.demo.toastError"))}
        >
          {t("dashboard.demo.toastError")}
        </Button>
      ),
    },
    {
      label: t("dashboard.demo.confirm"),
      hint: t("dashboard.demo.confirmHint"),
      node: (
        <Button size="sm" variant="destructive" onClick={() => void runConfirm()}>
          {t("dashboard.demo.confirm")}
        </Button>
      ),
    },
  ];

  return (
    <div className="col-span-12 lg:col-span-6">
      <Card>
        <CardHeader>
          <CardTitle>{t("dashboard.demo.title")}</CardTitle>
          <CardDescription>{t("dashboard.demo.description")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          {rows.map((row) => (
            <div key={row.label} className="flex items-center gap-3">
              <div className="w-40 shrink-0">{row.node}</div>
              <span className="text-muted-foreground text-xs">{row.hint}</span>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
