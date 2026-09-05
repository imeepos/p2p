import { useEffect } from "react";
import { useLocation, Outlet } from "react-router-dom";

import { rememberNetworkTab, tabIdForPath } from "./network-tab-memory";
import { NetworkTabBar } from "./network-tabs";

// /network tab 化容器（docs/design/app-shell-redesign.md 4.1）：子路由即
// tab，容器只渲染 tab 条 + Outlet；当前 tab 写入会话级记忆，供索引重定向
// 落回最后停留 tab。tab 条不随切换重挂载（兄弟子路由共享本容器）。
export function NetworkPage() {
  const { pathname } = useLocation();

  useEffect(() => {
    const id = tabIdForPath(pathname);
    if (id) rememberNetworkTab(id);
  }, [pathname]);

  return (
    <>
      <NetworkTabBar />
      <Outlet />
    </>
  );
}
