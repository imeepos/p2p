import { TriangleAlertIcon } from "lucide-react";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";

const PREFIX_LENGTH = 4;

interface ResetDialogBodyProps {
  prefixInput: string;
  prefixMatches: boolean;
  onPrefixChange: (value: string) => void;
  onCancel: () => void;
  confirmReset: () => Promise<unknown>;
  onResetSuccess: () => void;
  onResetError: (error: unknown) => void;
}

function ResetDialogBody({
  prefixInput,
  prefixMatches,
  onPrefixChange,
  onCancel,
  confirmReset,
  onResetSuccess,
  onResetError,
}: ResetDialogBodyProps) {
  const { t } = useTranslation();

  return (
    <>
      <AlertDialogHeader>
        <AlertDialogTitle>{t("settings.identity.resetTitle")}</AlertDialogTitle>
        <AlertDialogDescription>
          {t("settings.identity.resetDesc")}
        </AlertDialogDescription>
      </AlertDialogHeader>
      <Input
        value={prefixInput}
        placeholder={t("settings.identity.resetInputHint")}
        onChange={(event) => onPrefixChange(event.target.value)}
      />
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          {t("common.actions.cancel")}
        </Button>
        <AsyncButton
          type="button"
          variant="destructive"
          action={confirmReset}
          onSuccess={onResetSuccess}
          onError={onResetError}
          disabled={!prefixMatches}
          loadingLabel={t("settings.identity.resetting")}
        >
          {t("settings.identity.resetConfirm")}
        </AsyncButton>
      </div>
    </>
  );
}

// identity_reset 危险链路：确认弹框内输入 PeerId 前 4 位方可执行。
// 失败时弹框留驻，fail 态驻留结束后可直接重试。
export function ResetIdentityDialog({ peerId }: { peerId: string | null }) {
  const { t } = useTranslation();
  const [prefixInput, setPrefixInput] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const prefixMatches =
    peerId !== null && prefixInput === peerId.slice(0, PREFIX_LENGTH);

  const confirmReset = useCallback(async () => {
    await ipc.identityReset(true);
    await useNodeStore.getState().refresh();
  }, []);

  const handleResetSuccess = useCallback(() => {
    toastSuccess(t("settings.identity.resetDone"));
    setDialogOpen(false);
    setPrefixInput("");
  }, [t]);

  const handleResetError = useCallback(
    (error: unknown) => {
      console.error("[settings] identity_reset 失败", error);
      toastError(t("settings.identity.resetFailed"), errorText(error));
    },
    [t],
  );

  return (
    <AlertDialog
      open={dialogOpen}
      onOpenChange={(next) => {
        setDialogOpen(next);
        if (!next) setPrefixInput("");
      }}
    >
      <AlertDialogTrigger asChild>
        <Button
          type="button"
          variant="destructive"
          className="w-fit"
          disabled={peerId === null}
        >
          <TriangleAlertIcon aria-hidden />
          {t("settings.identity.reset")}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <ResetDialogBody
          prefixInput={prefixInput}
          prefixMatches={prefixMatches}
          onPrefixChange={setPrefixInput}
          onCancel={() => setDialogOpen(false)}
          confirmReset={confirmReset}
          onResetSuccess={handleResetSuccess}
          onResetError={handleResetError}
        />
      </AlertDialogContent>
    </AlertDialog>
  );
}
