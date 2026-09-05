// 通讯录分节锚点常量（§3.1）：锚点 id 固定 friends/groups/agents，与
// 占位页契约及命令面板 /contacts#* 深链一致；独立文件承载非组件导出
// （react-refresh/only-export-components）。
export const CONTACTS_SECTIONS = ["friends", "groups", "agents"] as const;

export type ContactsSectionId = (typeof CONTACTS_SECTIONS)[number];

export function isContactsSectionId(value: string | null): value is ContactsSectionId {
  return (CONTACTS_SECTIONS as readonly string[]).includes(value ?? "");
}
