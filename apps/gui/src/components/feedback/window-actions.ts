// 兜底页出路动作：ErrorBoundary 挂载点在 Router 之外（main.tsx），不能依赖
// useNavigate，必须用与 Router 无关的原生导航。独立成模块便于测试替身——
// jsdom 的 window.location 属性不可配置，无法直接 spyOn。
export function goToHomeRoute(): void {
  // HashRouter 下 "#/" 即首页；无 Router 上下文时仅改变地址，无副作用
  window.location.hash = "#/";
}

export function reloadWindow(): void {
  window.location.reload();
}
