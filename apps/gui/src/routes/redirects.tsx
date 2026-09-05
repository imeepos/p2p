import { Navigate, useLocation } from "react-router-dom";

// 5.3 统一透传函数：旧路由 query 原样并入重定向目标，目标自身参数优先
// （/group?x=1 -> /chat?kind=group&x=1）。全部重定向路由必须经此处构造
// 目标，禁止在路由树各处手写拼接。
export function mergeRedirectQuery(target: string, search: string): string {
  const [path, targetQuery] = target.split("?");
  const merged = new URLSearchParams(targetQuery);
  for (const [key, value] of new URLSearchParams(search)) {
    if (!merged.has(key)) merged.append(key, value);
  }
  const query = merged.toString();
  return query ? `${path}?${query}` : path;
}

// 5.3 重定向层（常驻中间路由，非迁移期临时物）：路由树集中注册后，
// hash router 内部导航完成重定向，不引入全页刷新；replace 使历史栈
// 不残留旧地址。
export function QueryRedirect({ to }: { to: string }) {
  const { search } = useLocation();
  return <Navigate to={mergeRedirectQuery(to, search)} replace />;
}
