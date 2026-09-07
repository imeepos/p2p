// offer show 未发布语义识别（R2-03/R2-26）：从未发布属常态空态而非故障，
// 空态走中文出路文案、console 静默；仅真实错误才显示错误原文并告警。
// mock 与 live（ai-guide offer show）同以 "never published" 语义报错，
// 中文变体兜底后端本地化文案（硬编码 CJK 门禁：以 \u 转义书写匹配片段，
// 非用户可见文案）；不匹配者一律按真实错误处理。
const NOT_PUBLISHED_RE =
  /never published|not published|\u4ece\u672a\u53d1\u5e03|\u5c1a\u672a\u53d1\u5e03/i;

export function isOfferNotPublished(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return NOT_PUBLISHED_RE.test(message);
}

// R2-26：console 降噪——真实错误整个会话仅提示一次。独立于组件文件导出，
// 避免 react-refresh 混导出。
let loadWarned = false;

/** 测试专用：重置会话级告警标记（模块单例用例隔离入口，resetToastDedupForTest 惯例） */
export function resetOfferLoadWarnForTest(): void {
  loadWarned = false;
}

export function warnOfferLoadOnce(error: unknown): void {
  if (loadWarned) return;
  loadWarned = true;
  console.warn("[llm-share] offer show 失败", error);
}
