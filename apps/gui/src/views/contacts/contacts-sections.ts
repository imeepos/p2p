import { createContext, useContext } from "react";

import type { ContactSelection } from "./contacts-detail-model";

// 通讯录分节锚点常量（§3.1）：锚点 id 固定 friends/groups/agents，与
// 占位页契约及命令面板 /contacts#* 深链一致；独立文件承载非组件导出
// （react-refresh/only-export-components）。
export const CONTACTS_SECTIONS = ["friends", "groups", "agents"] as const;

export type ContactsSectionId = (typeof CONTACTS_SECTIONS)[number];

export function isContactsSectionId(value: string | null): value is ContactsSectionId {
  return (CONTACTS_SECTIONS as readonly string[]).includes(value ?? "");
}

// 子串匹配（大小写不敏感）：fields 任一命中即保留；空白检索词全保留。
// P2#8 各节检索共用；非组件导出独立成文件（react-refresh 约束）。
export function matchesQuery(
  fields: Array<string | null | undefined>,
  query: string,
): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return fields.some((field) => (field ?? "").toLowerCase().includes(q));
}

// —— 双栏改版（微信通讯录式）：左栏树 + 右栏资料卡。检索词/选中项/折叠/
// 锚点为跨节状态，经 Context 下发；无 Provider（节组件单挂测试）时回退
// 惰性默认值：不过滤、未选中、全展开、锚点不动。
export interface ContactsPaneApi {
  query: string;
  selectedKey: string | null;
  select: (next: ContactSelection) => void;
  isSectionCollapsed: (id: string) => boolean;
  toggleSection: (id: string) => void;
  activeSection: ContactsSectionId | null;
  gotoSection: (id: ContactsSectionId) => void;
}

export const ContactsPaneContext = createContext<ContactsPaneApi>({
  query: "",
  selectedKey: null,
  select: () => {},
  isSectionCollapsed: () => false,
  toggleSection: () => {},
  activeSection: null,
  gotoSection: () => {},
});

export function useContactsPane(): ContactsPaneApi {
  return useContext(ContactsPaneContext);
}

// 分节折叠记忆：localStorage 损坏/不可用时回退全展开并留 warn（不静默）。
const SECTION_COLLAPSE_KEY = "contacts.tree.collapsed";

export function loadCollapsedSections(): Set<string> {
  try {
    const raw = window.localStorage.getItem(SECTION_COLLAPSE_KEY);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? new Set(parsed.filter((x): x is string => typeof x === "string"))
      : new Set();
  } catch (error) {
    console.warn("[contacts] 分节折叠态读取失败，按全展开处理", error);
    return new Set();
  }
}

export function saveCollapsedSections(sections: Set<string>): void {
  try {
    window.localStorage.setItem(SECTION_COLLAPSE_KEY, JSON.stringify([...sections]));
  } catch (error) {
    console.warn("[contacts] 分节折叠态保存失败", error);
  }
}
