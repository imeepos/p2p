# M2 预备调研（2026-09-15，M1 轮顺手采集）

## 屏幕采集（macOS）
- [screencapturekit-rs](https://github.com/doom-fish/screencapturekit-rs)：ScreenCaptureKit 的
  safe/idiomatic Rust 绑定，支持屏/窗/音频采集与区域截取；macOS 12.3+。
  候选 rd-capture 主依赖（商用优先 SCK，CGDisplayStream 兜底旧系统）。
- 屏幕录制授权：SCK 需 TCC 屏幕录制权限；缺失时首帧失败，须显式报错
  （沿用 gui screenshot 的 CAPTURE_PERMISSION_DENIED 形态先例）。
- 帧管线：SCK 输出 CVPixelBuffer（BGRA），转 RGBA 后走 rd-wire /rd/video/1 帧信封；
  首版 codec=raw/zlib，M6 后评估硬件 H.264（VideoToolbox）。

## 输入注入（macOS）
- [cgevents](https://docs.rs/crate/cgevents)（0.5.x）：CGEvent 的 Rust 绑定，
  鼠标移动/按键/滚轮 + 修饰键；需辅助功能（Accessibility）授权，
  `AXIsProcessTrusted` 探测缺失即显式拒绝。
- 键码映射：viewer 侧 DOM key → USB HID usage id（u16，已入 rd-wire input_key）→
  macOS virtual keycode（CGKeyCode）映射表随 rd-input 落地并单测。

## 待 M2 开工时验证
- screencapturekit-rs 的 region/delta 截取 API 形状与帧率控制；
- cgevents 的 Cargo 版本与 macOS 版本兼容面；
- 两库在 rust 1.98.1 / macOS 本机（Tauri 壳）下的构建可行性。
