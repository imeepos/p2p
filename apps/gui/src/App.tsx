import {
  createHashRouter,
  createRoutesFromChildren,
  Route,
  RouterProvider,
} from "react-router-dom";
import type { ReactNode } from "react";

import { AppLayout } from "@/components/layout/app-layout";
import { ChatRoutePage } from "@/routes/chat-route";
import { ContactsPage } from "@/routes/contacts-page";
import { DocsPage } from "@/routes/docs-page";
import { MessagesPage } from "@/views/messages/messages-page";
import { DiagnosticsPage } from "@/routes/diagnostics-page";
import { DiscoveryPage } from "@/routes/discovery-page";
import { EventsPage } from "@/routes/events-page";
import { AcpManagePage } from "@/routes/acp-manage-page";
import { LlmSharePage } from "@/routes/llm-share-page";
import { NetworkIndexRedirect } from "@/routes/network-index-redirect";
import { NetworkOverviewPage } from "@/routes/network-overview-page";
import { PeersPage } from "@/routes/peers-page";
import { QueryRedirect } from "@/routes/redirects";
import { RelayPage } from "@/routes/relay-page";
import { SettingsPage } from "@/routes/settings-page";
import { NetworkPage } from "@/views/network/network-page";
import { UnsavedRouteGuard } from "@/views/shared/unsaved-guard";

// 路由树（中央登记，注册改动压独立小提交）：MENU_ENTRIES 四入口与路由
// 一一对应；/network/* 子路由即 tab（4.1），由 NetworkPage 容器承载 tab
// 条，索引落最后停留 tab（会话记忆，全新会话落 overview）；5.3 旧路由
// 重定向为常驻中间路由，query 经 redirects.tsx 统一透传函数合并。
// data router（createHashRouter + createRoutesFromChildren）为 useBlocker 前提：
// 未保存守卫在路由层拦截，rail 点击与 Cmd/Ctrl+数字快捷键两条导航路径统一覆盖。
function guarded(element: ReactNode): ReactNode {
  return <UnsavedRouteGuard>{element}</UnsavedRouteGuard>;
}

const routes = createRoutesFromChildren(
  <Route element={<AppLayout />}>
    <Route index element={<QueryRedirect to="/network/overview" />} />
    <Route path="network">
      <Route index element={<NetworkIndexRedirect />} />
      <Route element={<NetworkPage />}>
        <Route path="overview" element={<NetworkOverviewPage />} />
        <Route path="peers" element={<PeersPage />} />
        <Route path="discovery" element={guarded(<DiscoveryPage />)} />
        <Route path="relay" element={guarded(<RelayPage />)} />
        <Route path="events" element={<EventsPage />} />
        <Route path="diagnostics" element={<DiagnosticsPage />} />
      </Route>
    </Route>
    <Route path="chat" element={<ChatRoutePage />} />
    {/* IMC3：消息中心（入群/好友邀请），append-only 登记 */}
    <Route path="messages" element={<MessagesPage />} />
    <Route path="contacts" element={<ContactsPage />} />
    {/* DOC2：协议文档页（append-only 登记，rail 保持 4 项） */}
    <Route path="docs" element={<DocsPage />} />
    {/* LSG3：LLM 共享四面板（append-only 登记，rail 保持 4 项，命令面板可达） */}
    <Route path="llm-share" element={<LlmSharePage />} />
    {/* 本地 ACP 管理页（append-only 登记，rail 不动，命令面板与 ACP 视图入口可达） */}
    <Route path="acp-manage" element={<AcpManagePage />} />
    <Route path="settings" element={guarded(<SettingsPage />)} />
    {/* 5.3 重定向层：旧路由 → 新位置；/group /acp 落 /chat?kind=*（已拍板项 1） */}
    <Route path="peers" element={<QueryRedirect to="/network/peers" />} />
    <Route path="discovery" element={<QueryRedirect to="/network/discovery" />} />
    <Route path="relay" element={<QueryRedirect to="/network/relay" />} />
    <Route path="events" element={<QueryRedirect to="/network/events" />} />
    <Route path="diagnostics" element={<QueryRedirect to="/network/diagnostics" />} />
    <Route path="group" element={<QueryRedirect to="/chat?kind=group" />} />
    <Route path="acp" element={<QueryRedirect to="/chat?kind=agent" />} />
  </Route>,
);

const router = createHashRouter(routes);

export default function App() {
  return <RouterProvider router={router} />;
}
