// offer show 未发布语义识别（R2-03/R2-26）：从未发布属常态空态而非故障，
// 空态走中文出路文案、console 静默；仅真实错误才显示错误原文并告警。
// mock 与 live（ai-guide offer show）同以 "never published" 语义报错，
// 中文变体兜底后端本地化文案；不匹配者一律按真实错误处理。
const NOT_PUBLISHED_RE = /never published|not published|从未发布|尚未发布/i;

export function isOfferNotPublished(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return NOT_PUBLISHED_RE.test(message);
}
