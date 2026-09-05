import { useEffect } from "react";

// 脏状态守卫注册表（路由层拦截的声明侧）：设置表单 / 节点资料草稿 /
// 中继地址列表 / 发现页 mDNS 草稿在各视图注册，UnsavedRouteGuard 统一消费。
export interface UnsavedGuardHandler {
  hasUnsaved: () => boolean;
  discard: () => void;
}

const guards = new Map<string, UnsavedGuardHandler>();

export function registerUnsavedGuard(
  key: string,
  handler: UnsavedGuardHandler,
): () => void {
  guards.set(key, handler);
  return () => {
    if (guards.get(key) === handler) guards.delete(key);
  };
}

export function hasAnyUnsaved(): boolean {
  for (const handler of guards.values()) {
    if (handler.hasUnsaved()) return true;
  }
  return false;
}

// 放弃离开：各编辑面丢弃本地草稿（导航前同步执行，卸载后置 state 无副作用）
export function discardAllUnsaved(): void {
  for (const handler of guards.values()) handler.discard();
}

// 视图内注册：卸载自动注销；handler 引用随渲染更新时重注册，读取恒为最新闭包。
export function useUnsavedGuard(
  key: string,
  handler: UnsavedGuardHandler,
): void {
  useEffect(() => registerUnsavedGuard(key, handler), [key, handler]);
}
