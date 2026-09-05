import { ContactsView } from "@/views/contacts/contacts-view";

// /contacts 路由页（app-shell-redesign §3 通讯录，P2 行）：实作在
// views/contacts（三区 + 添加流 + agent 详情抽屉），本文件只承载路由挂载。
export function ContactsPage() {
  return <ContactsView />;
}
