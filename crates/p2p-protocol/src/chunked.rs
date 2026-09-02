//! chunked transfer（design §9 原语库）：单帧装不下的大 payload 分帧传输。
//!
//! 线格式：每个帧的 payload 首字节为类型头，其后是数据：
//! - [FRAME_SINGLE] `0x00`：整条消息一帧装下，消息到此结束
//! - [FRAME_CHUNK]  `0x01`：中间分片，后随更多帧
//! - [FRAME_END]    `0x02`：最后一个分片，读端收到即重组完成
//!
//! 每帧数据部分 ≤ [CHUNK_DATA_SIZE]（= MAX_FRAME_SIZE - 1，帧长仍受 1 MiB 上限约束）；
//! 重组总大小超过 [MAX_MESSAGE_SIZE] 返回 MessageTooLarge，防对端灌爆内存。

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::{read_frame, write_frame, ProtocolError, MAX_FRAME_SIZE};

pub const FRAME_SINGLE: u8 = 0x00;
pub const FRAME_CHUNK: u8 = 0x01;
pub const FRAME_END: u8 = 0x02;

/// 单帧内可携带的数据量（1 字节留给类型头）。
pub const CHUNK_DATA_SIZE: usize = MAX_FRAME_SIZE as usize - 1;
/// 重组消息的防御性上限。
pub const MAX_MESSAGE_SIZE: u64 = 64 << 20;

/// 把整条 payload 按 single/chunk/end 约定分帧写出。
pub async fn write_chunked(
    w: &mut (impl AsyncWrite + Unpin + Send),
    payload: &[u8],
) -> io::Result<()> {
    if payload.len() <= CHUNK_DATA_SIZE {
        return write_typed_frame(w, FRAME_SINGLE, payload).await;
    }
    let mut rest = payload;
    while rest.len() > CHUNK_DATA_SIZE {
        let (head, tail) = rest.split_at(CHUNK_DATA_SIZE);
        write_typed_frame(w, FRAME_CHUNK, head).await?;
        rest = tail;
    }
    write_typed_frame(w, FRAME_END, rest).await
}

async fn write_typed_frame(
    w: &mut (impl AsyncWrite + Unpin + Send),
    kind: u8,
    data: &[u8],
) -> io::Result<()> {
    let mut frame = Vec::with_capacity(1 + data.len());
    frame.push(kind);
    frame.extend_from_slice(data);
    write_frame(w, &frame).await
}

/// 读端重组：读到 SINGLE/END 帧即返回完整消息。
/// 类型序非法（如 SINGLE 出现在分片中途）返回 InvalidData。
pub async fn read_chunked(r: &mut (impl AsyncRead + Unpin + Send)) -> io::Result<Vec<u8>> {
    let mut msg: Vec<u8> = Vec::new();
    loop {
        let frame = read_frame(r).await?;
        let Some((&kind, data)) = frame.split_first() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "chunked frame missing type byte",
            ));
        };
        let total = msg.len() as u64 + data.len() as u64;
        if total > MAX_MESSAGE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                ProtocolError::MessageTooLarge(total),
            ));
        }
        match kind {
            FRAME_SINGLE if msg.is_empty() => return Ok(data.to_vec()),
            FRAME_CHUNK => msg.extend_from_slice(data),
            FRAME_END => {
                msg.extend_from_slice(data);
                return Ok(msg);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "unexpected chunked frame type {kind:#04x} after {} bytes",
                        msg.len()
                    ),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten_io;
    use tokio::io::duplex;

    #[tokio::test]
    async fn small_payload_roundtrips() {
        let (mut tx, mut rx) = duplex(1024);
        let payload = b"tiny".to_vec();
        let (w, r) = tokio::join!(write_chunked(&mut tx, &payload), read_chunked(&mut rx));
        w.unwrap();
        assert_eq!(r.unwrap(), payload);
    }

    #[tokio::test]
    async fn empty_payload_roundtrips() {
        let (mut tx, mut rx) = duplex(64);
        let (w, r) = tokio::join!(write_chunked(&mut tx, &[]), read_chunked(&mut rx));
        w.unwrap();
        assert!(r.unwrap().is_empty());
    }

    #[tokio::test]
    async fn chunked_3mb_roundtrip() {
        let payload: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let (mut tx, mut rx) = duplex(64 * 1024);
        let (w, r) = tokio::join!(write_chunked(&mut tx, &payload), read_chunked(&mut rx));
        w.unwrap();
        assert_eq!(r.unwrap(), payload);
    }

    #[tokio::test]
    async fn oversize_total_rejected() {
        let (mut tx, mut rx) = duplex(64 * 1024);
        let payload = vec![7u8; MAX_MESSAGE_SIZE as usize + 1];
        let writer = tokio::spawn(async move { write_chunked(&mut tx, &payload).await });
        let err = read_chunked(&mut rx).await.unwrap_err();
        assert!(
            matches!(flatten_io(err), ProtocolError::MessageTooLarge(_)),
            "expected MessageTooLarge"
        );
        let _ = writer.await;
    }

    #[tokio::test]
    async fn unknown_type_header_rejected() {
        let (mut tx, mut rx) = duplex(64);
        write_frame(&mut tx, &[0x7f, 1, 2, 3]).await.unwrap();
        let err = read_chunked(&mut rx).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
