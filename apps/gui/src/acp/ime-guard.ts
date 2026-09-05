// IME 组合态判定：中文输入法选词期间的 Enter 是「确认候选词」，不得触发发送。
// isComposing 覆盖标准路径；keyCode 229 兜底个别浏览器/驱动不置 isComposing 的组合态
//（keydown 事件在 IME 处理中统一上报 229）。
export interface KeyboardEventLike {
  isComposing?: boolean | undefined;
  keyCode?: number | undefined;
}

export function isImeComposing(event: KeyboardEventLike): boolean {
  return event.isComposing === true || event.keyCode === 229;
}
