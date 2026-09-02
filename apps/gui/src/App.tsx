import { HashRouter, Route, Routes } from "react-router-dom";

import { AppLayout } from "@/components/layout/app-layout";
import { DashboardPage } from "@/routes/dashboard-page";
import { DiscoveryPage } from "@/routes/discovery-page";
import { EventsPage } from "@/routes/events-page";
import { PeersPage } from "@/routes/peers-page";
import { RelayPage } from "@/routes/relay-page";
import { SettingsPage } from "@/routes/settings-page";

// 路由注册与 menu.def.ts 一一对应；新增视图先改 menu.def 再补这里。
export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<DashboardPage />} />
          <Route path="peers" element={<PeersPage />} />
          <Route path="discovery" element={<DiscoveryPage />} />
          <Route path="relay" element={<RelayPage />} />
          <Route path="events" element={<EventsPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
