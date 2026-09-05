import { Navigate, useLocation } from "react-router-dom";

import { mergeRedirectQuery } from "./redirect-query";

// 5.3 重定向层（常驻中间路由，非迁移期临时物）：路由树集中注册后，
// hash router 内部导航完成重定向，不引入全页刷新；replace 使历史栈
// 不残留旧地址。
export function QueryRedirect({ to }: { to: string }) {
  const { search } = useLocation();
  return <Navigate to={mergeRedirectQuery(to, search)} replace />;
}
