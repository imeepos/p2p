import { toast } from "sonner";

import i18n from "@/i18n";

const SUCCESS_DURATION_MS = 3000;
const ERROR_DURATION_MS = 6000;
const DEDUP_WINDOW_MS = 3000;
const DEDUP_MAP_PRUNE_SIZE = 100;

export interface ToastErrorOptions {
  /** 失败原因单行摘要，展示在 toast 副文案 */
  description?: string;
  /** 复制到剪贴板的完整错误信息；缺省取 description */
  detail?: string;
  /** 操作上下文标识（IPC 通道或动作名），随详情一起复制 */
  context?: string;
}

const recentKeys = new Map<string, number>();

function isDuplicate(key: string): boolean {
  const now = Date.now();
  if (recentKeys.size > DEDUP_MAP_PRUNE_SIZE) {
    for (const [k, at] of recentKeys) {
      if (now - at >= DEDUP_WINDOW_MS) recentKeys.delete(k);
    }
  }
  const last = recentKeys.get(key);
  if (last !== undefined && now - last < DEDUP_WINDOW_MS) return true;
  recentKeys.set(key, now);
  return false;
}

export function toastSuccess(message: string, description?: string) {
  if (isDuplicate("ok:" + message)) {
    return toast.dismiss();
  }
  return toast.success(message, {
    description,
    duration: SUCCESS_DURATION_MS,
  });
}

// 复制内容含操作上下文与完整错误信息，便于粘贴到工单/终端排查。
export function buildErrorDetailClipboard(
  message: string,
  options: ToastErrorOptions,
): string {
  const lines: string[] = [];
  if (options.context) lines.push(`context: ${options.context}`);
  lines.push(`error: ${message}`);
  const detail = options.detail ?? options.description;
  if (detail) lines.push(`detail: ${detail}`);
  return lines.join("\n");
}

async function copyErrorDetail(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(i18n.t("common.copied"), { duration: SUCCESS_DURATION_MS });
  } catch (error) {
    console.error("[toast] 复制错误详情失败", error);
    toast.error(i18n.t("common.copyFailed"), { duration: ERROR_DURATION_MS });
  }
}

// 二参 string 为兼容形态（等价 { description }），存量调用点渐进迁移。
function normalizeErrorOptions(
  options?: string | ToastErrorOptions,
): ToastErrorOptions {
  if (typeof options === "string") return { description: options };
  return options ?? {};
}

export function toastError(
  message: string,
  options?: string | ToastErrorOptions,
) {
  const normalized = normalizeErrorOptions(options);
  if (isDuplicate("err:" + message)) {
    return toast.dismiss();
  }
  const clipboardText = buildErrorDetailClipboard(message, normalized);
  return toast.error(message, {
    description: normalized.description,
    duration: ERROR_DURATION_MS,
    action: {
      label: i18n.t("common.feedback.copyDetail"),
      onClick: () => void copyErrorDetail(clipboardText),
    },
  });
}
