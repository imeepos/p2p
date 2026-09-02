import { TriangleAlertIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Input } from "@/components/ui/input";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";

const PREFIX_LENGTH = 4;

interface ResetDialogBodyProps {
  prefixInput: string;
  resetting: boolean;
  prefixMatches: boolean;
  onPrefixChange: (value: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

function ResetDialogBody({
  prefixInput,
  resetting,
  prefixMatches,
  onPrefixChange,
  onCancel,
  onConfirm,
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
        <Button
          type="button"
          variant="destructive"
          disabled={!prefixMatches || resetting}
          onClick={onConfirm}
        >
          {t("settings.identity.resetConfirm")}
        </Button>
      </div>
    </>
  );
}

async function executeReset(doneMsg: string, failMsg: string, onDone: () => void) {
  try {
    await ipc.identityReset(true);
    await useNodeStore.getState().refresh();
    toastSuccess(doneMsg);
    onDone();
  } catch (error) {
    console.error("[settings] identity_reset 失败", error);
    toastError(failMsg, errorText(error));
  }
}

// identity_reset 危险链路：确认弹框内输入 PeerId 前 4 位方可执行。
export function ResetIdentityDialog({ peerId }: { peerId: string | null }) {
  const { t } = useTranslation();
  const [prefixInput, setPrefixInput] = useState("");
  const [resetting, setResetting] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const prefixMatches =
    peerId !== null && prefixInput === peerId.slice(0, PREFIX_LENGTH);

  const doReset = async () => {
    if (!prefixMatches || resetting) return;
    setResetting(true);
    await executeReset(
      t("settings.identity.resetDone"),
      t("settings.identity.resetFailed"),
      () => {
        setDialogOpen(false);
        setPrefixInput("");
      },
    );
    setResetting(false);
  };

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
          resetting={resetting}
          prefixMatches={prefixMatches}
          onPrefixChange={setPrefixInput}
          onCancel={() => setDialogOpen(false)}
          onConfirm={() => void doReset()}
        />
      </AlertDialogContent>
    </AlertDialog>
  );
}
