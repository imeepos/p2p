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
import { DashboardPage } from "@/routes/dashboard-page";
import { DiagnosticsPage } from "@/routes/diagnostics-page";
import { DiscoveryPage } from "@/routes/discovery-page";
import { EventsPage } from "@/routes/events-page";
import { PeersPage } from "@/routes/peers-page";
import { QueryRedirect } from "@/routes/redirects";
import { RelayPage } from "@/routes/relay-page";
import { SettingsPage } from "@/routes/settings-page";
import { UnsavedRouteGuard } from "@/views/shared/unsaved-guard";

// 路由树（中央登记，注册改动压独立小提交）：MENU_ENTRIES 四入口与路由
// 一一对应；/network/* 子路由 P0 先直接挂旧视图组件（git mv 整体迁移归 P3，
// docs/design/app-shell-redesign.md 1.2/六）；5.3 旧路由重定向为常驻中间
// 路由，query 经 redirects.tsx 统一透传函数合并。
// data router（createHashRouter + createRoutesFromChildren）为 useBlocker 前提：
// 未保存守卫在路由层拦截，rail 点击与 Cmd/Ctrl+数字快捷键两条导航路径统一覆盖。
function guarded(element: ReactNode): ReactNode {
  return <UnsavedRouteGuard>{element}</UnsavedRouteGuard>;
}

const routes = createRoutesFromChildren(
  <Route element={<AppLayout />}>
    <Route index element={<QueryRedirect to="/network/overview" />} />
    <Route path="network">
      <Route index element={<QueryRedirect to="/network/overview" />} />
      <Route path="overview" element={<DashboardPage />} />
      <Route path="peers" element={<PeersPage />} />
      <Route path="discovery" element={guarded(<DiscoveryPage />)} />
      <Route path="relay" element={guarded(<RelayPage />)} />
      <Route path="events" element={<EventsPage />} />
      <Route path="diagnostics" element={<DiagnosticsPage />} />
    </Route>
    <Route path="chat" element={<ChatRoutePage />} />
    <Route path="contacts" element={<ContactsPage />} />
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
