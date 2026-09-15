//! /rd/video/1 视频帧信封（remote-desktop-plan §3.2）。
//! 手写小端二进制编解码：紧凑优先；解码端对非法输入一律拒绝，不猜测。

use crate::WireError;

pub const MAGIC: u8 = 0x52; // 'R'
pub const FLAG_KEYFRAME: u8 = 0x01;
pub const CODEC_RAW_RGBA: u8 = 0;
pub const CODEC_ZLIB_RGBA: u8 = 1;
/// 单边尺寸上限（防御性）。
pub const MAX_DIM: u16 = 16_384;
/// 矩形块数量上限（防解码循环放大）。
pub const MAX_RECTS: usize = 256;
/// 载荷上限（含压缩），防对端灌爆内存。
pub const MAX_PAYLOAD_BYTES: u32 = 256 << 20;

/// 脏区矩形（帧内坐标，x+w ≤ 帧宽）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// 帧头：keyframe 为同步点，viewer 收到后重绘全帧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub seq: u32,
    pub ts_ms: u64,
    pub w: u16,
    pub h: u16,
    pub keyframe: bool,
    pub codec: u8,
    pub rects: Vec<Rect>,
}

/// 编码：信封 + payload 整体产出（>1 MiB 由调用方走 chunked）。
pub fn encode_frame(header: &FrameHeader, payload: &[u8]) -> Result<Vec<u8>, WireError> {
    validate_header(header, payload.len())?;
    let rect_bytes = header.rects.len() * 8;
    let mut out = Vec::with_capacity(24 + rect_bytes + 4 + payload.len());
    out.push(MAGIC);
    out.push(crate::PROTOCOL_VERSION);
    out.extend_from_slice(&header.seq.to_le_bytes());
    out.extend_from_slice(&header.ts_ms.to_le_bytes());
    out.extend_from_slice(&header.w.to_le_bytes());
    out.extend_from_slice(&header.h.to_le_bytes());
    let flags = if header.keyframe { FLAG_KEYFRAME } else { 0 };
    out.push(flags);
    out.push(header.codec);
    let n = header.rects.len();
    out.extend_from_slice(&(n as u16).to_le_bytes());
    for rect in &header.rects {
        out.extend_from_slice(&rect.x.to_le_bytes());
        out.extend_from_slice(&rect.y.to_le_bytes());
        out.extend_from_slice(&rect.w.to_le_bytes());
        out.extend_from_slice(&rect.h.to_le_bytes());
    }
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// 解码：返回帧头 + 载荷；字节数不足/越界约束即错误。
pub fn decode_frame(bytes: &[u8]) -> Result<(FrameHeader, Vec<u8>), WireError> {
    let mut r = Reader { buf: bytes, pos: 0 };
    let magic = r.take_u8()?;
    if magic != MAGIC {
        return Err(WireError::Invalid(format!("bad video magic {magic:#04x}")));
    }
    let ver = r.take_u8()?;
    if ver != crate::PROTOCOL_VERSION {
        return Err(WireError::Invalid(format!("bad video version {ver}")));
    }
    let seq = r.take_u32()?;
    let ts_ms = r.take_u64()?;
    let w = r.take_u16()?;
    let h = r.take_u16()?;
    let flags = r.take_u8()?;
    let codec = r.take_u8()?;
    let n = r.take_u16()? as usize;
    if n > MAX_RECTS {
        return Err(WireError::Invalid(format!("too many rects {n}")));
    }
    let mut rects = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.take_u16()?;
        let y = r.take_u16()?;
        let rw = r.take_u16()?;
        let rh = r.take_u16()?;
        rects.push(Rect { x, y, w: rw, h: rh });
    }
    let payload_len = r.take_u32()? as usize;
    let payload = r.take_slice(payload_len)?.to_vec();
    let header = FrameHeader {
        seq,
        ts_ms,
        w,
        h,
        keyframe: flags & FLAG_KEYFRAME != 0,
        codec,
        rects,
    };
    validate_header(&header, payload.len())?;
    Ok((header, payload))
}

fn validate_header(header: &FrameHeader, payload_len: usize) -> Result<(), WireError> {
    if header.w == 0 || header.h == 0 || header.w > MAX_DIM || header.h > MAX_DIM {
        return Err(WireError::Invalid(format!(
            "bad dims {}x{}",
            header.w, header.h
        )));
    }
    if header.codec != CODEC_RAW_RGBA && header.codec != CODEC_ZLIB_RGBA {
        return Err(WireError::UnsupportedCodec(header.codec));
    }
    if header.rects.len() > MAX_RECTS {
        return Err(WireError::Invalid("too many rects".into()));
    }
    for rect in &header.rects {
        if rect.w == 0
            || rect.h == 0
            || rect.x as u32 + rect.w as u32 > header.w as u32
            || rect.y as u32 + rect.h as u32 > header.h as u32
        {
            return Err(WireError::Invalid("rect out of frame bounds".into()));
        }
    }
    if payload_len > MAX_PAYLOAD_BYTES as usize {
        return Err(WireError::Invalid("payload too large".into()));
    }
    Ok(())
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take_u8(&mut self) -> Result<u8, WireError> {
        self.take_slice(1).map(|s| s[0])
    }

    fn take_u16(&mut self) -> Result<u16, WireError> {
        let s = self.take_slice(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    fn take_u32(&mut self) -> Result<u32, WireError> {
        let s = self.take_slice(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn take_u64(&mut self) -> Result<u64, WireError> {
        let s = self.take_slice(8)?;
        Ok(u64::from_le_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    fn take_slice(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(WireError::Truncated(self.pos))?;
        if end > self.buf.len() {
            return Err(WireError::Truncated(self.pos));
        }
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }
}
