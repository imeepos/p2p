import i18n from "@/i18n";
import { toastError } from "@/components/feedback/toast";

// 发送命令 Ok 但报告失败（IM-T51）：后端 mark_failed 路径 chatSend 仍返回
// Ok(report)，composer 的 catch 不会触发——在此统一上浮 toast，失败禁止零解释。
// 结构化最小面：1:1 ChatSendReport 与群 GroupSendReport 共用同一判定。
// transport 注入面返回 unknown（A2A 会话无 report，2026-09-09 实证 undefined
// 曾在此炸出 TypeError 并误报「发送失败」）：非报告形状一律放行不算失败。
export interface FailedSendReportLike {
  delivered: boolean;
  message: { status: string; peer?: string; id?: string };
}

export function isFailedSendReport(report: unknown): boolean {
  if (typeof report !== "object" || report === null) return false;
  const r = report as Partial<FailedSendReportLike>;
  if (typeof r.delivered !== "boolean") return false;
  if (typeof r.message !== "object" || r.message === null) return false;
  return !r.delivered && r.message.status === "failed";
}

export function notifyFailedSendReport(report: unknown): void {
  if (!isFailedSendReport(report)) return;
  const failed = report as FailedSendReportLike;
  console.error(
    "[chat] send failed (mark_failed):",
    failed.message.peer ?? "(group)",
    failed.message.id ?? "",
  );
  toastError(i18n.t("chat.sendFailedReason"), { context: "chat.send" });
}
