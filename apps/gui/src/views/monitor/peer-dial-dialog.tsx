import { useTranslation } from "react-i18next";
import { useState } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { DialReport } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { parseDialTarget } from "./dial-target";
import { DialDialogFooter } from "./dial-dialog-footer";
import { DialResultPanel } from "./dial-result-panel";
import { DialTargetField } from "./dial-target-field";

interface PeerDialDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 手动拨号：契约 §6 格式校验（内联红字），提交后 Dialog 内展示逐跳结果。
export function PeerDialDialog({ open, onOpenChange }: PeerDialDialogProps) {
  const { t } = useTranslation();
  const dial = useNodeStore((s) => s.dial);
  const [target, setTarget] = useState("");
  const [report, setReport] = useState<DialReport | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

  const invalid = target.length > 0 && parseDialTarget(target) === null;

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setTarget("");
      setReport(null);
      setCommandError(null);
    }
    onOpenChange(next);
  };

  const submit = async () => {
    const parsed = parseDialTarget(target);
    if (!parsed) throw new Error(t("peers.dial.invalidFormat"));
    const result = await dial(`${parsed.peerId}@${parsed.addr}`);
    setReport(result);
    return result;
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("peers.dial.title")}</DialogTitle>
          <DialogDescription>
            {t("peers.dial.targetPlaceholder")}
          </DialogDescription>
        </DialogHeader>
        <DialTargetField
          target={target}
          onTargetChange={setTarget}
          invalid={invalid}
          commandError={commandError}
        />
        {report && <DialResultPanel report={report} />}
        <DialDialogFooter
          canSubmit={target.length > 0}
          onClose={() => handleOpenChange(false)}
          onSubmit={submit}
          onCommandError={setCommandError}
        />
      </DialogContent>
    </Dialog>
  );
}
