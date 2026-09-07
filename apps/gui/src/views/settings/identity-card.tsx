import { CopyIcon } from "lucide-react";
import { useFormContext } from "react-hook-form";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { useNodeStore } from "@/stores/node-store";
import { copyText } from "@/views/shared/clipboard";
import type { SettingsFormValues } from "./config-schema";
import { ResetIdentityDialog } from "./reset-identity-dialog";
import { SettingsGroup, SettingsRow } from "./settings-row";

// 身份组：PeerId 展示/复制、数据目录、重置身份（输入前 4 位双重确认）。
export function IdentityCard() {
  const { t } = useTranslation();
  const { watch } = useFormContext<SettingsFormValues>();
  const peerId = useNodeStore((s) => s.status?.peerId ?? null);
  const dataDir = watch("dataDir");

  return (
    <SettingsGroup
      title={t("settings.cards.identity")}
      description={t("settings.identity.hint")}
    >
      <SettingsRow
        label={t("common.labels.peerId")}
        control={
          <div className="flex items-center gap-2">
            <code
              className="bg-muted w-56 truncate rounded px-2 py-1 font-mono text-xs"
              title={peerId ?? undefined}
            >
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
        }
      />
      <SettingsRow
        label={t("settings.identity.dataDir")}
        control={
          <code
            className="bg-muted max-w-72 truncate rounded px-2 py-1 font-mono text-xs"
            title={dataDir || undefined}
          >
            {dataDir || "-"}
          </code>
        }
      />
      <SettingsRow
        label={t("settings.identity.danger")}
        description={
          peerId === null ? t("settings.identity.peerIdUnavailable") : undefined
        }
        control={<ResetIdentityDialog peerId={peerId ?? ""} />}
      />
    </SettingsGroup>
  );
}
