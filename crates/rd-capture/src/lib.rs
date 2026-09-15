//! rd-capture：屏幕采集抽象（remote-desktop-plan §2.3）。
//!
//! [CaptureSource] 是 host 侧视频泵的唯一接缝：合成源（测试/E2E）与
//! macOS ScreenCaptureKit 真实源（[macos::ScreenKitSource]）可互换；
//! 权限缺失等失败路径以 [CaptureError] 显式上抛，禁止静默吞错。

pub mod synthetic;

#[cfg(all(target_os = "macos", feature = "sck"))]
pub mod macos;

/// 一帧全屏画面：RGBA8（字节序 R,G,B,A）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub w: u16,
    pub h: u16,
    pub rgba: Vec<u8>,
}

impl CapturedFrame {
    /// 帧字节数校验：w/h 非零且与 rgba 长度一致（编解码前 MUST 先过）。
    pub fn validate(&self) -> Result<(), CaptureError> {
        if self.w == 0 || self.h == 0 {
            return Err(CaptureError::InvalidFrame("zero dimension".into()));
        }
        let expect = (self.w as u32 * self.h as u32) as usize * 4;
        if self.rgba.len() != expect {
            return Err(CaptureError::InvalidFrame(format!(
                "rgba len {} != {w}x{h}x4",
                self.rgba.len(),
                w = self.w,
                h = self.h
            )));
        }
        Ok(())
    }
}

/// 采集失败：可观测、可归因。
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("screen recording permission denied")]
    PermissionDenied,
    #[error("no display available")]
    NoDisplay,
    #[error("capture stream: {0}")]
    Stream(String),
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    #[error("io: {0}")]
    Io(String),
}

/// 采集源接缝：host 视频泵逐帧拉取；实现须返回最新一帧，阻塞即跳帧语义由宿主负责。
pub trait CaptureSource: Send {
    /// 抓一帧；失败以显式错误上抛（权限缺失/流中断等）。
    fn next_frame(&mut self) -> Result<CapturedFrame, CaptureError>;

    /// 当前分辨率（w,h）；未知（真实源首帧前）可返回 (0,0)。
    fn resolution(&self) -> (u16, u16);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_validate_rejects_bad_len() {
        let ok = CapturedFrame {
            w: 2,
            h: 2,
            rgba: vec![0u8; 16],
        };
        assert!(ok.validate().is_ok());
        let bad = CapturedFrame {
            w: 2,
            h: 2,
            rgba: vec![0u8; 15],
        };
        assert!(bad.validate().is_err());
        let zero = CapturedFrame {
            w: 0,
            h: 2,
            rgba: vec![],
        };
        assert!(zero.validate().is_err());
    }
}
