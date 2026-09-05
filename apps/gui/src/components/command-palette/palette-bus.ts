// 命令面板打开请求的壳内事件通道：面板 open 状态归 AppLayout 受控所有，
// 顶栏等兄弟组件经此总线请求打开，避免布局组件向子组件反向钻状态。
const OPEN_EVENT = "p2p:command-palette:open";

export function requestOpenCommandPalette(): void {
  window.dispatchEvent(new CustomEvent(OPEN_EVENT));
}

export function subscribeOpenCommandPalette(handler: () => void): () => void {
  const listener = () => handler();
  window.addEventListener(OPEN_EVENT, listener);
  return () => window.removeEventListener(OPEN_EVENT, listener);
}
