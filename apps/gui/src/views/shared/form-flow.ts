// 控制流标记：校验失败/用户取消时以特定错误中断 AsyncButton，
// onError 据此跳过错误 toast（校验错误已内联展示，取消不是故障）。
export const FORM_VALIDATION_MARK = "form-validation-blocked";
export const ACTION_CANCELLED_MARK = "action-cancelled";

export function isFlowMark(error: unknown, mark: string): boolean {
  return error instanceof Error && error.message === mark;
}

export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
