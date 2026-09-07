// R2-12：校验失败可见化——按传入顺序聚焦第一个无效字段并滚动入视，
// 返回错误字段数。与 settings focus-first-error 同理念；本卡为自管校验
// （非 react-hook-form），有效性由调用方同步判定，避免读 DOM 的渲染竞态。
export function focusFirstInvalidField(
  fieldIds: string[],
  isInvalid: (id: string) => boolean,
): number {
  const invalidIds = fieldIds.filter(isInvalid);
  if (invalidIds.length === 0) return 0;
  const element = document.getElementById(invalidIds[0]!);
  if (element) {
    element.focus({ preventScroll: true });
    // jsdom 无 scrollIntoView 实现：能力探测降级为仅聚焦
    if (typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  } else {
    console.error("[llm-share] 校验失败字段未找到 DOM 节点:", invalidIds[0]);
  }
  return invalidIds.length;
}
