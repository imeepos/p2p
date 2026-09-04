//! ndjson 分块层：纯字节，不解析 JSON 语义（设计 §4.2-1/§4.2-3）。
//! 背压 = 调用方逐帧驱动，本层不排队；缓冲有界，超护栏断流并毒化。

use crate::consts::{FRAME_CHUNK_LIMIT, LINE_GUARD_LIMIT};
use crate::error::ErrorCode;

const NEWLINE: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

/// 写侧：把一行（不含行尾换行）切成帧序列——内容帧各 ≤ FRAME_CHUNK_LIMIT，
/// 行尾 '\n' 以独立 1 字节帧收尾，对端按字节流即可还原 ndjson 行界。零拷贝。
/// 约定：line 不含 '\n'；interior '\n' 属上游违例，原样透传将在对端拆行。
pub fn frames(line: &[u8]) -> Frames<'_> {
    Frames {
        rest: line,
        newline_pending: true,
    }
}

pub struct Frames<'a> {
    rest: &'a [u8],
    newline_pending: bool,
}

impl<'a> Iterator for Frames<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            if self.newline_pending {
                self.newline_pending = false;
                return Some(&[NEWLINE]);
            }
            return None;
        }
        let take = self.rest.len().min(FRAME_CHUNK_LIMIT);
        let (head, tail) = self.rest.split_at(take);
        self.rest = tail;
        Some(head)
    }
}

/// 读侧重组器：push_frame 累积帧、take_line 逐行取出。
/// 产出行含 '\n' 结尾；'\r\n' 归一为 '\n'；空行原样透传（b"\n"）。
pub struct LineReassembler {
    buf: Vec<u8>,
    poisoned: Option<ErrorCode>,
}

impl Default for LineReassembler {
    fn default() -> Self {
        Self {
            buf: Vec::with_capacity(FRAME_CHUNK_LIMIT),
            poisoned: None,
        }
    }
}

impl LineReassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一帧。帧超 FRAME_CHUNK_LIMIT → FrameTooLarge（底座已限帧，此处防御复核）；
    /// 累积超 LINE_GUARD_LIMIT → LineTooLong（携带上限值）。两者均毒化断流。
    pub fn push_frame(&mut self, frame: &[u8]) -> Result<(), ErrorCode> {
        if let Some(err) = self.poisoned {
            return Err(err);
        }
        if frame.len() > FRAME_CHUNK_LIMIT {
            eprintln!(
                "acp-common: inbound frame {} bytes > chunk limit, stream poisoned",
                frame.len()
            );
            let err = ErrorCode::FrameTooLarge {
                size_bytes: frame.len(),
                limit_bytes: FRAME_CHUNK_LIMIT,
            };
            self.poison(err);
            return Err(err);
        }
        if self.buf.len() + frame.len() > LINE_GUARD_LIMIT {
            eprintln!(
                "acp-common: buffered {} + frame {} bytes > line guard, stream poisoned",
                self.buf.len(),
                frame.len()
            );
            let err = ErrorCode::LineTooLong {
                limit_bytes: LINE_GUARD_LIMIT,
            };
            self.poison(err);
            return Err(err);
        }
        self.buf.extend_from_slice(frame);
        Ok(())
    }

    /// 取出一条完整行；无完整行返回 None。
    pub fn take_line(&mut self) -> Option<Vec<u8>> {
        let pos = self.buf.iter().position(|&b| b == NEWLINE)?;
        let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
        if line.len() >= 2 && line[line.len() - 2] == CARRIAGE_RETURN {
            line.remove(line.len() - 2);
        }
        Some(line)
    }

    /// 流收尾：残留未换行字节 = 上游死在半行，显式报错不静默丢弃。
    pub fn finish(&mut self) -> Result<(), ErrorCode> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            eprintln!(
                "acp-common: stream ended with {} buffered bytes, line dropped",
                self.buf.len()
            );
            let err = ErrorCode::NdjsonTruncated;
            self.poison(err);
            Err(err)
        }
    }

    /// 毒化原因；Some 即必须废弃本重组器（断流语义）。
    pub fn poisoned(&self) -> Option<ErrorCode> {
        self.poisoned
    }

    fn poison(&mut self, err: ErrorCode) {
        self.poisoned = Some(err);
        self.buf = Vec::new();
    }
}
