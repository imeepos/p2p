//! 请求体流式读取：Content-Length 截断与 chunked 解码，内存占用恒定。
//!
//! 桥只为单请求单连接（应答恒 Connection: close），读完请求头后把
//! OwnedReadHalf 交给 BodyReader；fill() 是普通 async 方法，避免手写
//! poll_read 状态机。

use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyMode {
    Empty,
    Length(u64),
    Chunked,
}

pub struct BodyReader {
    inner: Box<dyn AsyncRead + Unpin + Send>,
    mode: BodyMode,
    chunk_remaining: u64,
    finished: bool,
}

impl BodyReader {
    pub fn empty() -> Self {
        Self {
            inner: Box::new(tokio::io::empty()),
            mode: BodyMode::Empty,
            chunk_remaining: 0,
            finished: true,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        Self {
            inner: Box::new(std::io::Cursor::new(bytes)),
            mode: BodyMode::Length(len),
            chunk_remaining: len,
            finished: false,
        }
    }

    pub fn from_stream(
        inner: Box<dyn AsyncRead + Unpin + Send>,
        content_length: Option<u64>,
    ) -> Self {
        let mode = match content_length {
            Some(n) => BodyMode::Length(n),
            None => BodyMode::Chunked,
        };
        Self {
            inner,
            mode,
            chunk_remaining: 0,
            finished: matches!(mode, BodyMode::Empty),
        }
    }

    /// 声明的剩余字节数（chunked 为 None）。
    pub fn remaining(&self) -> Option<u64> {
        match self.mode {
            BodyMode::Length(n) => Some(n),
            BodyMode::Chunked => None,
            BodyMode::Empty => Some(0),
        }
    }

    /// 读至多 out.len() 字节；Ok(0) = 请求体结束。
    pub async fn fill(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.finished || out.is_empty() {
            return Ok(0);
        }
        let n = match self.mode {
            BodyMode::Empty => 0,
            BodyMode::Length(remaining) => {
                let cap = (remaining.min(out.len() as u64)) as usize;
                let n = self.inner.read(&mut out[..cap]).await?;
                self.mode = BodyMode::Length(remaining - n as u64);
                n
            }
            BodyMode::Chunked => {
                if self.chunk_remaining == 0 {
                    self.next_chunk_header().await?;
                    if self.finished {
                        return Ok(0);
                    }
                }
                let cap = (self.chunk_remaining.min(out.len() as u64)) as usize;
                let n = self.inner.read(&mut out[..cap]).await?;
                if n == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "chunked body truncated",
                    ));
                }
                self.chunk_remaining -= n as u64;
                if self.chunk_remaining == 0 {
                    self.consume_chunk_terminator().await?;
                }
                n
            }
        };
        if matches!(self.mode, BodyMode::Length(0)) {
            self.finished = true;
        }
        Ok(n)
    }

    async fn next_chunk_header(&mut self) -> std::io::Result<()> {
        let line = read_line(&mut self.inner, 1024).await?;
        let size_part = line.split(';').next().unwrap_or("").trim();
        let size = u64::from_str_radix(size_part, 16).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("bad chunk size: {size_part}"),
            )
        })?;
        if size == 0 {
            // 尾随头块：读到空行为止（连接随后关闭，无需保存）。
            loop {
                let l = read_line(&mut self.inner, 1024).await?;
                if l.is_empty() {
                    break;
                }
            }
            self.finished = true;
            return Ok(());
        }
        self.chunk_remaining = size;
        Ok(())
    }

    async fn consume_chunk_terminator(&mut self) -> std::io::Result<()> {
        let mut crlf = [0u8; 2];
        self.inner.read_exact(&mut crlf).await?;
        if &crlf != b"\r\n" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "chunk terminator missing",
            ));
        }
        Ok(())
    }
}

async fn read_line(r: &mut (impl AsyncRead + Unpin), cap: usize) -> std::io::Result<String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = r.read(&mut byte).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "line truncated",
            ));
        }
        if byte[0] == b'\n' {
            break;
        }
        buf.push(byte[0]);
        if buf.len() > cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "line too long",
            ));
        }
    }
    let s = String::from_utf8_lossy(&buf);
    Ok(s.trim_end_matches('\r').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(bytes: &[u8]) -> Box<dyn AsyncRead + Unpin + Send> {
        Box::new(std::io::Cursor::new(bytes.to_vec()))
    }

    #[tokio::test]
    async fn length_body_fill_and_eof() {
        let mut body = BodyReader::from_stream(stream(b"hello"), Some(5));
        let mut out = [0u8; 3];
        assert_eq!(body.fill(&mut out).await.unwrap(), 3);
        assert_eq!(&out, b"hel");
        let mut rest = [0u8; 8];
        assert_eq!(body.fill(&mut rest).await.unwrap(), 2);
        assert_eq!(body.fill(&mut rest).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn chunked_body_decode() {
        let raw = b"3\r\nabc\r\n4;ext\r\ndefg\r\n0\r\nX-T: v\r\n\r\n";
        let mut body = BodyReader::from_stream(stream(raw), None);
        let mut got = Vec::new();
        let mut buf = [0u8; 2];
        loop {
            let n = body.fill(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, b"abcdefg");
    }

    #[tokio::test]
    async fn chunked_truncated_rejected() {
        let mut body = BodyReader::from_stream(stream(b"5\r\nabc"), None);
        let mut buf = [0u8; 8];
        // 先正常收到 3 字节；再读时体被截断必须显式报错（禁静默当 EOF）。
        assert_eq!(body.fill(&mut buf).await.unwrap(), 3);
        assert!(body.fill(&mut buf).await.is_err());
    }
}
