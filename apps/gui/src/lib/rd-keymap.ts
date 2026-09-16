// DOM 键鼠事件 → rd 线协议输入编码（rd-control.md：HID usage id + 位掩码）。
// 映射表只收常用键；未映射键返回 null，由调用方决定忽略并提示。

/** modifiers：0x1 shift / 0x2 ctrl / 0x4 alt / 0x8 meta。 */
export function modifiersOf(event: {
  shiftKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): number {
  let m = 0;
  if (event.shiftKey) m |= 0x1;
  if (event.ctrlKey) m |= 0x2;
  if (event.altKey) m |= 0x4;
  if (event.metaKey) m |= 0x8;
  return m;
}

/** buttons：bit0 左 / bit1 右 / bit2 中（rd-control.md 消息表）。 */
export function buttonsOf(event: { buttons: number }): number {
  let b = 0;
  if (event.buttons & 1) b |= 0x1;
  if (event.buttons & 2) b |= 0x4;
  if (event.buttons & 4) b |= 0x2;
  return b;
}

/** 单键按下：按 e.button 编号映射位。 */
export function buttonBitOf(event: { button: number }): number {
  switch (event.button) {
    case 0:
      return 0x1;
    case 2:
      return 0x4;
    case 1:
      return 0x2;
    default:
      return 0;
  }
}

/** 滚轮增量收敛到 i8（协议 wheel_dx/dy 为 i8，一行一个方向不越界）。 */
export function clampWheel(delta: number): number {
  if (!Number.isFinite(delta)) return 0;
  const q = Math.trunc(delta);
  return Math.max(-128, Math.min(127, q));
}

// USB HID Usage ID（Keyboard/Keypad page 0x07）常用键映射；DOM code 为键位（不随布局变）。
const HID_CODES: Record<string, number> = {
  Space: 0x2c,
  Enter: 0x28,
  Escape: 0x29,
  Backspace: 0x2a,
  Tab: 0x2b,
  CapsLock: 0x39,
  ArrowUp: 0x52,
  ArrowDown: 0x51,
  ArrowLeft: 0x50,
  ArrowRight: 0x4f,
  Delete: 0x4c,
  Insert: 0x49,
  Home: 0x4a,
  End: 0x4d,
  PageUp: 0x4b,
  PageDown: 0x4e,
  ShiftLeft: 0xe1,
  ShiftRight: 0xe5,
  ControlLeft: 0xe0,
  ControlRight: 0xe4,
  AltLeft: 0xe2,
  AltRight: 0xe6,
  MetaLeft: 0xe3,
  MetaRight: 0xe7,
};

/** DOM code → HID usage id；未收录返回 null。 */
export function hidCodeOf(code: string): number | null {
  if (HID_CODES[code] !== undefined) return HID_CODES[code];
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return 0x04 + (letter[1].charCodeAt(0) - 65);
  const digit = /^Digit([1-9])$/.exec(code);
  if (digit) return 0x1e + (Number(digit[1]) - 1);
  if (code === "Digit0") return 0x27;
  const fn = /^F([1-9]|1[0-2])$/.exec(code);
  if (fn) return 0x3a + (Number(fn[1]) - 1);
  return null;
}
