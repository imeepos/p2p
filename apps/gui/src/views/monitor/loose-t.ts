import type { I18nKey } from "@/i18n/types";

// i18next 严格类型 t 不接受「动态 key + 通用 values」组合；
// 事件摘要等模板化场景收口为松散签名，运行时行为与 t 完全一致。
export type LooseT = (key: I18nKey, values?: Record<string, string>) => string;

export function toLooseT(t: unknown): LooseT {
  return t as LooseT;
}
