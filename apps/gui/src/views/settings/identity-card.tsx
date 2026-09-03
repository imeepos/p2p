import { CopyIcon } from "lucide-react";
import { useFormContext } from "react-hook-form";
import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { useNodeStore } from "@/stores/node-store";
import { copyText } from "@/views/shared/clipboard";
import type { SettingsFormValues } from "./config-schema";
import { ResetIdentityDialog } from "./reset-identity-dialog";

function PeerIdRow({ peerId }: { peerId: string | null }) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-1">
      <Label>{t("common.labels.peerId")}</Label>
      <div className="flex items-center gap-2">
        <code className="bg-muted min-w-0 flex-1 truncate rounded px-2 py-1 font-mono text-xs">
          {peerId ?? t("settings.identity.peerIdUnavailable")}
        </code>
        <Button
          type="button"
          variant="outline"
          size="icon"
          disabled={!peerId}
          aria-label={t("common.actions.copy")}
          onClick={() => {
            if (peerId) {
              void copyText(peerId, {
                done: t("settings.identity.copyDone"),
                failed: t("settings.identity.copyFailed"),
              });
            }
          }}
        >
          <CopyIcon aria-hidden />
        </Button>
      </div>
    </div>
  );
}

// 身份卡：PeerId 展示/复制、数据目录、重置身份（输入前 4 位双重确认）。
export function IdentityCard() {
  const { t } = useTranslation();
  const { watch } = useFormContext<SettingsFormValues>();
  const peerId = useNodeStore((s) => s.status?.peerId ?? null);
  const dataDir = watch("dataDir");

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.identity")}</CardTitle>
        <CardDescription>{t("settings.identity.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <PeerIdRow peerId={peerId} />
        <div className="flex flex-col gap-1">
          <Label>{t("settings.identity.dataDir")}</Label>
          <code
            className="bg-muted block min-w-0 truncate rounded px-2 py-1 font-mono text-xs"
            title={dataDir || undefined}
          >
            {dataDir || "-"}
          </code>
        </div>
        <div className="flex flex-col gap-1">
          <Label>{t("settings.identity.danger")}</Label>
          <ResetIdentityDialog peerId={peerId ?? ""} />
          {peerId === null ? (
            <p className="text-muted-foreground text-xs">
              {t("settings.identity.peerIdUnavailable")}
            </p>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}