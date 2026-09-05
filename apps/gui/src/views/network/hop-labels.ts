import type { DialHopKind } from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";

// 逐跳标签 i18n key 映射：hop-timeline 与事件摘要共用（组件文件不可再导出常量）。
export const HOP_KEY: Record<DialHopKind, I18nKey> = {
  direct: "common.hop.direct",
  punch: "common.hop.punch",
  relay: "common.hop.relay",
};
