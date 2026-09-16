//! viewer 帧→webview 中继：DecodedFrame 打包帧头经 tauri Channel 二进制推送。
//!
//! 帧格式见 gui-contract §21.4：`[w:u16 LE][h:u16 LE][seq:u32 LE][rgba8]`；
//! 推送速率 = host 质量档 fps；发送失败记 warn（可观测，不静默）。

use rd_viewer::DecodedFrame;
use tauri::ipc::{Channel, InvokeResponseBody};

/// 帧头字节数：w(2) + h(2) + seq(4)。
pub const FRAME_HEADER_LEN: usize = 8;

/// 编码一帧为通道二进制载荷（帧头 + RGBA）。
pub fn encode_frame(frame: &DecodedFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER_LEN + frame.rgba.len());
    out.extend_from_slice(&frame.w.to_le_bytes());
    out.extend_from_slice(&frame.h.to_le_bytes());
    out.extend_from_slice(&frame.seq.to_le_bytes());
    out.extend_from_slice(&frame.rgba);
    out
}

/// RenderSink 的 webview 实现：每帧打包后经 Channel 推给前端 canvas。
pub struct WebviewSink {
    channel: Channel,
}

impl WebviewSink {
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }
}

impl rd_viewer::RenderSink for WebviewSink {
    fn on_frame(&self, frame: DecodedFrame) {
        if let Err(e) = self
            .channel
            .send(InvokeResponseBody::from(encode_frame(&frame)))
        {
            tracing::warn!(error = %e, "rd viewer 帧推送失败（webview 通道）");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_frame_header_and_payload() {
        let frame = DecodedFrame {
            w: 320,
            h: 180,
            keyframe: true,
            seq: 42,
            rgba: vec![1, 2, 3, 4],
        };
        let buf = encode_frame(&frame);
        assert_eq!(buf.len(), FRAME_HEADER_LEN + 4);
        let mut want_head = [0u8; 8];
        want_head[..2].copy_from_slice(&320u16.to_le_bytes());
        want_head[2..4].copy_from_slice(&180u16.to_le_bytes());
        want_head[4..].copy_from_slice(&42u32.to_le_bytes());
        assert_eq!(&buf[..8], &want_head);
        assert_eq!(&buf[8..], &[1, 2, 3, 4]);
    }

    #[test]
    fn encode_frame_roundtrip_header_fields() {
        let frame = DecodedFrame {
            w: 640,
            h: 360,
            keyframe: false,
            seq: 7,
            rgba: vec![0; 64],
        };
        let buf = encode_frame(&frame);
        let w = u16::from_le_bytes([buf[0], buf[1]]);
        let h = u16::from_le_bytes([buf[2], buf[3]]);
        let seq = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        assert_eq!((w, h, seq), (640, 360, 7));
        assert_eq!(buf.len(), FRAME_HEADER_LEN + 64);
    }
}
