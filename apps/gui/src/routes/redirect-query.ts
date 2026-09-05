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
