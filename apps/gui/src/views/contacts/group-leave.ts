import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { errorText } from "@/views/shared/form-flow";

// 退群流（通讯录行与资料卡共用）：第三档单次确认 → store.leave →
// 成功/失败 toast；退群在途按群锁定防连点。失败留日志与 toast，不静默。
export function useGroupLeave() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const leave = useGroupStore((s) => s.leave);
  const [leavingId, setLeavingId] = useState<string | null>(null);

  const leaveGroup = async (group: GroupJson) => {
    const ok = await confirm({
      title: t("contacts.groups.leaveConfirmTitle"),
      description: t("contacts.groups.leaveConfirmDesc", { group: group.name }),
      confirmText: t("contacts.groups.leaveConfirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    setLeavingId(group.groupId);
    try {
      await leave(group.groupId);
      toastSuccess(t("contacts.groups.leaveSuccess"));
    } catch (error) {
      console.error("[contacts] 退群失败", group.groupId, error);
      toastError(t("contacts.groups.leaveFailed"), {
        description: errorText(error),
        context: "group_leave",
      });
    } finally {
      setLeavingId(null);
    }
  };

  return { leaveGroup, leavingId };
}
