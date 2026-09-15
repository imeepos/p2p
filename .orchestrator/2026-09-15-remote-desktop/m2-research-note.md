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

## M2 实测结论（2026-09-15）
- screencapturekit 10.0.3 编译通过（rust 1.98.1 aarch64），API：SCShareableContent →
  SCContentFilter/SCStreamConfiguration → SCStream::add_output_handler(closure) →
  CMSampleBufferExt::pixel_buffer → CVPixelBuffer lock_read_only + base_address 逐行拷贝。
- **运行时硬约束**：SCK 依赖 Swift 运行时（libswift_Concurrency），本机 macOS 26.6
  的 dyld cache 缺该库 → 测试二进制加载即 SIGABRT。处置：rd-capture 的 SCK 模块
  feature 门控（`sck`，默认关），工作区门禁零 Swift 依赖；真实采集在带授权 + Swift
  运行时真机验证（GUI 波）。
- M3 结论：注入实现选 core-graphics 0.24（servo 系纯 Rust C-FFI，无 Swift 依赖，
  macOS 全版本可用；CGEvent 构造/投递 + KeyCode 常量 + CGEventFlags 齐备），
  弃 cgevents（同类 API，生态更小）。Accessibility 授权经 AXIsProcessTrusted FFI 探测。
