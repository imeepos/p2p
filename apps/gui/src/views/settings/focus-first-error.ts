import type { FieldErrors } from "react-hook-form";

import type { SettingsFormValues } from "./config-schema";

// 字段顺序与 config-schema 的表单结构一致：取「第一个错误」按此优先级。
const FIELD_ORDER: Array<keyof SettingsFormValues> = [
  "quicPort",
  "tcpPort",
  "bootstrap",
  "relayAddrs",
  "advertisedAddrs",
  "observationPort",
  "observationAddrs",
];

// DOM 定位：端口/开关用既有字段 id；地址列表由所在卡以 data-field 标注容器。
const FIELD_SELECTORS: Partial<Record<keyof SettingsFormValues, string>> = {
  quicPort: "#settings-quic-port",
  tcpPort: "#settings-tcp-port",
  enableMdns: "#settings-mdns",
  observationPort: "#settings-observation-port",
  bootstrap: '[data-field="bootstrap"] input',
  relayAddrs: '[data-field="relayAddrs"] input',
  advertisedAddrs: '[data-field="advertisedAddrs"] input',
  observationAddrs: '[data-field="observationAddrs"] input',
};

// 校验失败可见化：统计错误字段数，滚动并聚焦第一个错误字段。
// 返回错误字段数供保存条展示；定位不到 DOM 节点时留 console 信号不静默。
export function focusFirstInvalidField(
  errors: FieldErrors<SettingsFormValues>,
): number {
  const invalidFields = FIELD_ORDER.filter((field) => errors[field] != null);
  const first = invalidFields[0];
  if (!first) return 0;
  const selector = FIELD_SELECTORS[first];
  const element = selector
    ? document.querySelector<HTMLElement>(selector)
    : null;
  if (element) {
    element.focus({ preventScroll: true });
    element.scrollIntoView({ behavior: "smooth", block: "center" });
  } else {
    console.error("[settings] 校验失败字段未找到 DOM 节点:", first);
  }
  return invalidFields.length;
}
