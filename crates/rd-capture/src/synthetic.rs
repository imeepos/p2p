//! 确定性合成采集源：E2E 与单测专用，不依赖任何系统权限。
//! 像素由 (x,y,帧号) 哈希式图案生成，测试可逐帧校验内容完整性。

use crate::{CaptureError, CaptureSource, CapturedFrame};

/// 合成源：w×h RGBA，第 n 帧像素 = f(x,y,n) 确定性图案。
pub struct SyntheticSource {
    w: u16,
    h: u16,
    frame: u32,
}

impl SyntheticSource {
    pub fn new(w: u16, h: u16) -> Self {
        Self { w, h, frame: 0 }
    }

    /// 当前帧号（首帧为 1）。
    pub fn frame_index(&self) -> u32 {
        self.frame
    }

    /// 校验 viewer 收到的第 n 帧是否与源图案一致（E2E 完整性断言）。
    pub fn expect_frame(n: u32, w: u16, h: u16, rgba: &[u8]) -> bool {
        if rgba.len() != (w as usize) * (h as usize) * 4 {
            return false;
        }
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = (y * w as usize + x) * 4;
                let want = [pix(x, y, n, 3), pix(x, y, n, 5), pix(x, y, n, 7), 255];
                if rgba[i..i + 4] != want {
                    return false;
                }
            }
        }
        true
    }
}

fn pix(x: usize, y: usize, n: u32, k: u32) -> u8 {
    let v = x
        .wrapping_mul(3)
        .wrapping_add(y.wrapping_mul(5))
        .wrapping_add((n.wrapping_mul(k)) as usize);
    (v & 0xff) as u8
}

impl CaptureSource for SyntheticSource {
    fn next_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        self.frame = self.frame.wrapping_add(1);
        let n = self.frame;
        let total = self.w as usize * self.h as usize;
        let mut rgba = vec![0u8; total * 4];
        for y in 0..self.h as usize {
            for x in 0..self.w as usize {
                let i = (y * self.w as usize + x) * 4;
                rgba[i] = pix(x, y, n, 3);
                rgba[i + 1] = pix(x, y, n, 5);
                rgba[i + 2] = pix(x, y, n, 7);
                rgba[i + 3] = 255;
            }
        }
        Ok(CapturedFrame {
            w: self.w,
            h: self.h,
            rgba,
        })
    }

    fn resolution(&self) -> (u16, u16) {
        (self.w, self.h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic_and_distinct() {
        let mut src = SyntheticSource::new(16, 9);
        let f1 = src.next_frame().unwrap();
        let f2 = src.next_frame().unwrap();
        assert_ne!(f1.rgba, f2.rgba, "相邻帧图案必须不同");
        assert!(SyntheticSource::expect_frame(1, 16, 9, &f1.rgba));
        assert!(SyntheticSource::expect_frame(2, 16, 9, &f2.rgba));
        assert!(!SyntheticSource::expect_frame(2, 16, 9, &f1.rgba));
    }

    #[test]
    fn frame_index_tracks() {
        let mut src = SyntheticSource::new(4, 4);
        let _ = src.next_frame().unwrap();
        assert_eq!(src.frame_index(), 1);
    }
}
