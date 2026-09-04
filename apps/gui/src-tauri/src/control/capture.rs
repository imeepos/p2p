//! 帧源抽象 + PNG 编码/落盘 + macOS WKWebView 快照实现。
//!
//! 真实帧源走 WKWebView takeSnapshot（渲染窗口 webview 内容）；
//! 权限预检失败/零字节/异常图像一律结构化报错，禁止产出黑图或空文件（R3）。

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{Runtime, WebviewWindow};

/// 一帧 RGBA8 像素。
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct CaptureError {
    pub code: &'static str,
    pub message: String,
}

impl CaptureError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

pub trait FrameSource: Send + Sync {
    fn capture(&self) -> Result<Frame, CaptureError>;
}

pub fn encode_png(frame: &Frame) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut encoder = png::Encoder::new(&mut buf, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&frame.rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(buf)
}

/// 落盘前的最后防线：非 PNG / 空字节拒绝（R3 禁黑图空文件）。
pub fn ensure_png(bytes: &[u8]) -> Result<(), String> {
    const MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 33 || bytes[..8] != MAGIC {
        return Err(format!("输出不是合法 PNG（{} 字节），拒绝落盘", bytes.len()));
    }
    Ok(())
}

/// 临时文件 + 原子改名；返回字节数。空字节拒绝，失败清理临时文件。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<u64, String> {
    if bytes.is_empty() {
        return Err("输出为空字节，拒绝落盘".to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建输出目录 {} 失败: {e}", parent.display()))?;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("gc1");
    let tmp = path.with_file_name(format!(".{name}.part-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("写临时文件 {} 失败: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("改名 {} -> {} 失败: {e}", tmp.display(), path.display())
    })?;
    Ok(bytes.len() as u64)
}

/// 快照 PNG 字节 → RGBA 帧（RGB 输入就地扩为 RGBA）。
pub fn decode_png(bytes: &[u8]) -> Result<Frame, CaptureError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| CaptureError::new("CAPTURE_FAILED", format!("PNG 头解析失败: {e}")))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| CaptureError::new("CAPTURE_FAILED", format!("PNG 解码失败: {e}")))?;
    buf.truncate(info.buffer_size());
    if info.color_type == png::ColorType::Rgb {
        Ok(Frame { width: info.width, height: info.height, rgba: rgb_to_rgba(&buf) })
    } else {
        Ok(Frame { width: info.width, height: info.height, rgba: buf })
    }
}

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let px = rgb.len() / 3;
    let mut out = Vec::with_capacity(px * 4);
    for i in 0..px {
        out.extend_from_slice(&rgb[i * 3..i * 3 + 3]);
        out.push(255);
    }
    out
}

/// 真实帧源：捕获主窗口 webview 内容。
pub struct RealFrameSource<R: Runtime> {
    window: Option<WebviewWindow<R>>,
}

impl<R: Runtime> RealFrameSource<R> {
    pub fn new(window: Option<WebviewWindow<R>>) -> Self {
        Self { window }
    }
}

impl<R: Runtime> FrameSource for RealFrameSource<R> {
    #[cfg(target_os = "macos")]
    fn capture(&self) -> Result<Frame, CaptureError> {
        let Some(window) = &self.window else {
            return Err(CaptureError::new(
                "CAPTURE_UNAVAILABLE",
                "主窗口不存在（GUI 尚未就绪或已关闭）",
            ));
        };
        preflight_screen_permission()?;
        let (tx, rx) = std::sync::mpsc::channel();
        window
            .with_webview(move |platform| {
                // 闭包内只发起快照立即返回：完成回调（主队列）把结果送回 channel，
                // 等待只发生在服务线程的外层 recv，主线程不被内层阻塞饿死回调。
                unsafe { macos::request_snapshot(&platform, tx) };
            })
            .map_err(|e| CaptureError::new("CAPTURE_UNAVAILABLE", format!("webview 句柄派发失败: {e}")))?;
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(png)) => decode_png(&png),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CaptureError::new("CAPTURE_FAILED", "快照 5s 内未完成（回调未达），放弃本次截图")),
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn capture(&self) -> Result<Frame, CaptureError> {
        Err(CaptureError::new(
            "CAPTURE_UNAVAILABLE",
            "当前平台截图尚未实现（macOS 已支持）",
        ))
    }
}

/// 合成帧源：确定性棋盘格，随帧号变化（集成测试用，禁黑图断言可用）。
#[doc(hidden)]
pub struct SyntheticFrameSource {
    seq: AtomicU64,
    size: u32,
}

impl SyntheticFrameSource {
    pub fn new() -> Self {
        Self { seq: AtomicU64::new(0), size: 64 }
    }
}

impl Default for SyntheticFrameSource {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameSource for SyntheticFrameSource {
    fn capture(&self) -> Result<Frame, CaptureError> {
        let n = self.seq.fetch_add(1, Ordering::Relaxed) as u32;
        let w = self.size;
        let h = self.size;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 4) as u8;
                rgba[i + 2] = ((x ^ y).wrapping_add(n) * 7) as u8;
                rgba[i + 3] = 255;
            }
        }
        Ok(Frame { width: w, height: h, rgba })
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::mpsc::Sender;

    use block2::RcBlock;
    use objc2::{runtime::AnyObject, AnyThread};
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSImage,
    };
    use objc2_foundation::{NSDictionary, NSError};
    use objc2_web_kit::WKWebView;
    use tauri::webview::PlatformWebview;

    use super::CaptureError;

    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    /// R3：屏幕录制权限缺失 → 结构化错误，从源头杜绝黑图。
    pub fn preflight_screen_permission() -> Result<(), CaptureError> {
        if !unsafe { CGPreflightScreenCaptureAccess() } {
            return Err(CaptureError::new(
                "CAPTURE_PERMISSION_DENIED",
                "macOS 屏幕录制权限缺失：请在 系统设置 > 隐私与安全性 > 屏幕录制 中授权 GUI 后重试",
            ));
        }
        Ok(())
    }

    /// 发起 takeSnapshot：完成回调（WebKit 会 copy，主队列异步执行）持有 channel
    /// 发送端回传结果；闭包与调用线程都不等待，等待统一在服务线程外层 5s。
    pub unsafe fn request_snapshot(
        platform: &PlatformWebview,
        tx: Sender<Result<Vec<u8>, CaptureError>>,
    ) {
        let webview: &WKWebView = &*platform.inner().cast();
        let block = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
            send_snapshot(&tx, image, error);
        });
        webview.takeSnapshotWithConfiguration_completionHandler(None, &block);
    }

    fn send_snapshot(
        tx: &Sender<Result<Vec<u8>, CaptureError>>,
        image: *mut NSImage,
        error: *mut NSError,
    ) {
        let _ = tx.send(if image.is_null() {
            let detail = if error.is_null() {
                "快照返回空图像（无错误描述）".to_string()
            } else {
                let desc = unsafe { (*error).localizedDescription() }.to_string();
                format!("快照失败: {desc}")
            };
            Err(CaptureError::new("CAPTURE_FAILED", detail))
        } else {
            unsafe { tiff_to_png(&*image) }
        });
    }

    unsafe fn tiff_to_png(image: &NSImage) -> Result<Vec<u8>, CaptureError> {
        let tiff = image
            .TIFFRepresentation()
            .ok_or_else(|| CaptureError::new("CAPTURE_FAILED", "TIFF 导出失败"))?;
        let rep = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &tiff)
            .ok_or_else(|| CaptureError::new("CAPTURE_FAILED", "TIFF 转位图失败"))?;
        let empty = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::new();
        let data = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &empty)
            .ok_or_else(|| CaptureError::new("CAPTURE_FAILED", "快照 PNG 编码失败（AppKit）"))?;
        Ok(data.to_vec())
    }
}

#[cfg(target_os = "macos")]
use macos::preflight_screen_permission;
