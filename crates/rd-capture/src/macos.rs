//! macOS ScreenCaptureKit 真实采集源（macOS 13+，需屏幕录制 TCC 授权）。
//! 运行时权限缺失 → [CaptureError::PermissionDenied]（显式可观测，不吞错）。
//! 真实采集用例 #[ignore]：本机无授权时机械门禁不依赖系统权限。

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use screencapturekit::cm::CMSampleBufferExt;
use screencapturekit::prelude::*;

use crate::{CaptureError, CaptureSource, CapturedFrame};

/// SCK 真实采集源：start() 建流即开始输出，next_frame() 取最近帧。
pub struct ScreenKitSource {
    #[allow(dead_code)]
    stream: SCStream,
    rx: Receiver<CapturedFrame>,
}

impl ScreenKitSource {
    /// 启动采集：取首个显示器、原生分辨率、BGRA 输出、显示光标。
    pub fn start() -> Result<Self, CaptureError> {
        let content = SCShareableContent::get().map_err(map_sc_error)?;
        let display = content
            .displays()
            .into_iter()
            .next()
            .ok_or(CaptureError::NoDisplay)?;
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_windows(&[])
            .build();
        let config = SCStreamConfiguration::new()
            .with_pixel_format(PixelFormat::BGRA)
            .with_shows_cursor(true);
        let (tx, rx) = std::sync::mpsc::channel::<CapturedFrame>();
        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(
            move |sample: CMSampleBuffer, of_type: SCStreamOutputType| {
                if matches!(of_type, SCStreamOutputType::Screen) {
                    match sample_to_frame(&sample) {
                        Ok(frame) => {
                            let _ = tx.send(frame);
                        }
                        Err(e) => tracing::warn!("rd-capture: drop frame: {e}"),
                    }
                }
            },
            SCStreamOutputType::Screen,
        );
        stream.start_capture().map_err(map_sc_error)?;
        Ok(Self { stream, rx })
    }
}

impl CaptureSource for ScreenKitSource {
    fn next_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        match self.rx.recv_timeout(Duration::from_millis(1000)) {
            Ok(frame) => Ok(frame),
            Err(RecvTimeoutError::Timeout) => {
                Err(CaptureError::Stream("frame timeout (no SCK output)".into()))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(CaptureError::Stream("capture handler ended".into()))
            }
        }
    }

    fn resolution(&self) -> (u16, u16) {
        (0, 0)
    }
}

fn map_sc_error(e: screencapturekit::error::SCError) -> CaptureError {
    match e {
        screencapturekit::error::SCError::PermissionDenied(_)
        | screencapturekit::error::SCError::NoShareableContent(_) => CaptureError::PermissionDenied,
        other => CaptureError::Stream(other.to_string()),
    }
}

/// CMSampleBuffer → RGBA 帧：锁只读 base address，逐行 BGRA→RGBA 拷贝。
fn sample_to_frame(sample: &CMSampleBuffer) -> Result<CapturedFrame, CaptureError> {
    let buf = sample
        .pixel_buffer()
        .ok_or_else(|| CaptureError::Stream("sample has no pixel buffer".into()))?;
    let w = buf.width();
    let h = buf.height();
    if w == 0 || h == 0 || w > u16::MAX as usize || h > u16::MAX as usize {
        return Err(CaptureError::InvalidFrame(format!("bad size {w}x{h}")));
    }
    let bpr = buf.bytes_per_row();
    if w.checked_mul(4).is_none_or(|need| bpr < need) {
        return Err(CaptureError::InvalidFrame(format!("row too narrow {bpr}")));
    }
    if h.checked_mul(bpr)
        .is_none_or(|total| total > buf.data_size())
    {
        return Err(CaptureError::InvalidFrame(
            "buffer shorter than rows".into(),
        ));
    }
    let guard = buf
        .lock_read_only()
        .map_err(|e| CaptureError::Stream(format!("lock {e}")))?;
    let base = guard.base_address();
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        // 不变量：row 落在已校验的 [0, h*bpr) 缓冲区内，读 w*4 不越界。
        let row = unsafe { std::slice::from_raw_parts(base.add(y * bpr), w * 4) };
        for x in 0..w {
            let i = x * 4;
            let o = (y * w + x) * 4;
            rgba[o] = row[i + 2]; // BGRA -> RGBA
            rgba[o + 1] = row[i + 1];
            rgba[o + 2] = row[i];
            rgba[o + 3] = 255;
        }
    }
    Ok(CapturedFrame {
        w: w as u16,
        h: h as u16,
        rgba,
    })
}

/// 真实采集冒烟：需屏幕录制授权（本机无授权时显式 PermissionDenied 也算通过）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires macOS screen recording permission"]
    fn real_capture_smoke() {
        let mut src = match ScreenKitSource::start() {
            Ok(s) => s,
            Err(CaptureError::PermissionDenied) => return, // 无授权:环境限制,非实现回归
            Err(e) => panic!("unexpected start error: {e}"),
        };
        let frame = src.next_frame().expect("first frame");
        assert!(frame.validate().is_ok());
        assert!(frame.w > 0 && frame.h > 0);
    }
}
