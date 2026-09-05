// 快捷键徽标的平台差异：Apple 系显示 Cmd 系（⌘），其余显示 Ctrl 系。
// 判定用 navigator.platform 优先、userAgent 兜底，测试可替换 platform 模拟。
export function isApplePlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  const source = (navigator.platform ?? "") + " " + (navigator.userAgent ?? "");
  return /mac|iphone|ipad|ipod/i.test(source);
}

export function commandShortcutLabel(): string {
  return isApplePlatform() ? "\u2318K" : "Ctrl+K";
}
