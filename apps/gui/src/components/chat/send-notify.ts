import i18n from "@/i18n";
import { toastError } from "@/components/feedback/toast";
import type { ChatSendReport } from "@/lib/ipc-types";

// 发送命令 Ok 但报告失败（IM-T51）：后端 mark_failed 路径 chatSend 仍返回
// Ok(report)，composer 的 catch 不会触发——在此统一上浮 toast，失败禁止零解释。
export function isFailedSendReport(report: ChatSendReport): boolean {
  return !report.delivered && report.message.status === "failed";
}

export function notifyFailedSendReport(report: ChatSendReport): void {
  if (!isFailedSendReport(report)) return;
  console.error(
    "[chat] send failed (mark_failed):",
    report.message.peer,
    report.message.id,
  );
  toastError(i18n.t("chat.sendFailedReason"), { context: "chat.send" });
}
