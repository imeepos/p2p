import { PlusIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  isValidTransportAddr,
  noDuplicateAddrs,
} from "@/views/shared/address-rules";
import { ErrorText } from "@/views/shared/error-text";

interface AddDialogViewProps {
  draft: string;
  busy: boolean;
  error: string | undefined;
  onDraftChange: (value: string) => void;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}

function AddDialogView({
  draft,
  busy,
  error,
  onDraftChange,
  onCancel,
  onConfirm,
}: AddDialogViewProps) {
  const { t } = useTranslation();

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{t("discovery.rendezvous.addTitle")}</DialogTitle>
        <DialogDescription>
          {t("discovery.rendezvous.addDesc")}
        </DialogDescription>
      </DialogHeader>
      <div className="flex flex-col gap-1">
        <Input
          className="font-mono text-xs"
          placeholder="192.168.1.10/u3400"
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
        />
        <ErrorText code={error} />
      </div>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onCancel}>
          {t("common.actions.cancel")}
        </Button>
        <AsyncButton
          type="button"
          disabled={error !== undefined || busy || draft.trim().length === 0}
          action={onConfirm}
        >
          {t("common.actions.confirm")}
        </AsyncButton>
      </DialogFooter>
    </DialogContent>
  );
}

interface AddAddressDialogProps {
  existing: string[];
  saving: boolean;
  onAdd: (addr: string) => Promise<boolean>;
  /** 受控 open：传入时由父级控制开关（跨卡联动入口），缺省内部自管 */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

// 添加地址 Dialog：输入即校验（格式/重复内联红字），成功后关闭并清空。
export function AddAddressDialog({
  existing,
  saving,
  onAdd,
  open: openProp,
  onOpenChange,
}: AddAddressDialogProps) {
  const { t } = useTranslation();
  const [innerOpen, setInnerOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const dialogOpen = openProp ?? innerOpen;
  const setDialogOpen = onOpenChange ?? setInnerOpen;

  const trimmed = draft.trim();
  const draftError = trimmed.length === 0
    ? undefined
    : !isValidTransportAddr(trimmed)
      ? "addrFormat"
      : !noDuplicateAddrs([...existing, trimmed])
        ? "addrDuplicate"
        : undefined;

  // 保存失败由 discovery-view toast；校验错误内联红字：这里以异常中断按钮即可。
  const addDraft = async (): Promise<void> => {
    if (draftError !== undefined || saving) return;
    const saved = await onAdd(trimmed);
    if (!saved) throw new Error("bootstrap save failed");
    setDraft("");
    setDialogOpen(false);
  };

  return (
    <Dialog
      open={dialogOpen}
      onOpenChange={(next) => {
        setDialogOpen(next);
        if (!next) setDraft("");
      }}
    >
      <DialogTrigger asChild>
        <Button type="button" size="sm" variant="outline" className="w-fit">
          <PlusIcon aria-hidden />
          {t("discovery.rendezvous.add")}
        </Button>
      </DialogTrigger>
      <AddDialogView
        draft={draft}
        busy={saving}
        error={draftError}
        onDraftChange={setDraft}
        onCancel={() => setDialogOpen(false)}
        onConfirm={addDraft}
      />
    </Dialog>
  );
}
