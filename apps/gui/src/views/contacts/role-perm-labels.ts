import type { I18nKey } from "@/i18n/types";

// §4 权限闭集 key → i18n 标签映射；映射外 key 兜底显示原始 key
// （闭集扩充时 UI 不阻塞，勿用 t() 包裸 key）。
export const PERM_LABEL_KEYS: Record<string, I18nKey> = {
  "chat.send": "contacts.authz.manager.permChatSend",
  "chat.attachment": "contacts.authz.manager.permChatAttachment",
  "a2a.discover": "contacts.authz.manager.permA2aDiscover",
  "a2a.invoke": "contacts.authz.manager.permA2aInvoke",
  "acp.session": "contacts.authz.manager.permAcpSession",
  "acp.execute": "contacts.authz.manager.permAcpExecute",
  "llm.borrow": "contacts.authz.manager.permLlmBorrow",
  "repair.diag": "contacts.authz.manager.permRepairDiag",
  "repair.fix": "contacts.authz.manager.permRepairFix",
};

export function permLabel(permKey: string, translate: (key: I18nKey) => string): string {
  const mapped = PERM_LABEL_KEYS[permKey];
  return mapped ? translate(mapped) : permKey;
}
