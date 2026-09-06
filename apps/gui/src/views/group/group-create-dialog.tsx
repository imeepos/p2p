import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

import { GroupCreateForm } from "./group-create-form";

interface GroupCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 建群弹窗（F09）：表单抽到 GroupCreateForm 供本弹窗与联系人「添加群聊」
// 弹窗共用，两处一跳直达；表单状态随关闭卸载自然复位。
export function GroupCreateDialog({ open, onOpenChange }: GroupCreateDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="group-create-dialog">
        <DialogHeader>
          <DialogTitle>{t("group.create.title")}</DialogTitle>
          <DialogDescription>{t("group.description")}</DialogDescription>
        </DialogHeader>
        <GroupCreateForm onDone={() => onOpenChange(false)} />
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
