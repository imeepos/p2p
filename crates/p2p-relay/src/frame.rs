//! relay 帧编解码（design 5.2）：varint 长度前缀 + RelayMsg protobuf 负载。
//!
//! varint 语义与 p2p-protocol 对齐（审查 L4 同源问题）：最多 10 字节，
//! 第 10 字节（shift=63）仍带继续位、或数据位 > 1（<<63 丢高位回绕），
//! 一律 InvalidData，不静默回绕解码。

use prost::Message as ProstMessage;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::messages::RelayMsg;

/// 单帧上限（design 5.2 缺省 1 MiB）。
pub const MAX_FRAME: usize = 1 << 20;

fn push_varint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            buf.push(b);
            return;
        }
        buf.push(b | 0x80);
    }
}

/// 读无符号 varint；流在帧边界干净关闭时返回 None。
async fn read_varint(r: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<u64>> {
    let mut v = 0u64;
    let mut shift = 0u32;
    loop {
        let b = match r.read_u8().await {
            Ok(b) => b,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof && shift == 0 => return Ok(None),
            Err(e) => return Err(e),
        };
        let nibble = b & 0x7f;
        if shift == 63 && nibble > 1 {
            return Err(Error::new(ErrorKind::InvalidData, "varint overflow"));
        }
        v |= u64::from(nibble) << shift;
        if b & 0x80 == 0 {
            return Ok(Some(v));
        }
        shift += 7;
        if shift >= 64 {
            return Err(Error::new(ErrorKind::InvalidData, "varint overflow"));
        }
    }
}

/// 编码并写一帧。
pub async fn write_msg(w: &mut (impl AsyncWrite + Unpin), msg: &RelayMsg) -> std::io::Result<()> {
    let body = msg.encode_to_vec();
    if body.len() > MAX_FRAME {
        return Err(Error::new(ErrorKind::InvalidData, "relay frame too large"));
    }
    let mut buf = Vec::with_capacity(body.len() + 5);
    push_varint(&mut buf, body.len() as u64);
    buf.extend_from_slice(&body);
    w.write_all(&buf).await
}

/// 读一帧；None = 对端干净关流。
pub async fn read_msg(r: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<RelayMsg>> {
    let Some(len) = read_varint(r).await? else {
        return Ok(None);
    };
    if len as usize > MAX_FRAME {
        return Err(Error::new(ErrorKind::InvalidData, "relay frame too large"));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).await?;
    RelayMsg::decode(&body[..])
        .map(Some)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
}

/// 在流上写一条显式拒绝帧（失败路径必须留信号，不许静默）。
pub async fn write_reject(
    w: &mut (impl AsyncWrite + Unpin),
    code: u32,
    message: impl Into<String>,
) -> std::io::Result<()> {
    write_msg(w, &RelayMsg::error(code, message)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::errcode;

    fn samples() -> Vec<RelayMsg> {
        vec![
            RelayMsg::reserve(300, "peer-b"),
            RelayMsg::reserved(7),
            RelayMsg::connect(7),
            RelayMsg::bound(7),
            RelayMsg::punch_req("peer-b", vec!["1.2.3.4:5".into()]),
            RelayMsg::punch_ack("peer-a", vec!["5.6.7.8:9".into(), "[::1]:1".into()]),
            RelayMsg::error(errcode::UNKNOWN_CIRCUIT, "no such circuit"),
        ]
    }

    #[tokio::test]
    async fn roundtrip_all_kinds_over_duplex() {
        for msg in samples() {
            let (mut tx, mut rx) = tokio::io::duplex(256);
            write_msg(&mut tx, &msg).await.unwrap();
            let got = read_msg(&mut rx).await.unwrap().expect("one frame");
            assert_eq!(got, msg);
        }
    }

    #[tokio::test]
    async fn clean_close_reads_none() {
        let (tx, mut rx) = tokio::io::duplex(64);
        drop(tx);
        assert!(read_msg(&mut rx).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn oversized_length_prefix_rejected() {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let mut buf = Vec::new();
        push_varint(&mut buf, (MAX_FRAME + 1) as u64);
        tx.write_all(&buf).await.unwrap();
        let err = read_msg(&mut rx).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn varint_tenth_byte_overflow_rejected() {
        // 第 10 字节数据位 > 1（如 0x02）：<<63 会丢高位回绕，必须显式拒绝
        let (mut tx, mut rx) = tokio::io::duplex(64);
        tx.write_all(&[0xff; 9]).await.unwrap();
        tx.write_all(&[0x02]).await.unwrap();
        let err = read_varint(&mut rx).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);

        // 第 10 字节仍带继续位：拒绝且不读第 11 字节
        let (mut tx, mut rx) = tokio::io::duplex(64);
        tx.write_all(&[0xff; 10]).await.unwrap();
        let err = read_varint(&mut rx).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);

        // 合法边界：u64::MAX 恰为 10 字节（末字节 0x01），照常解码
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let mut buf = Vec::new();
        push_varint(&mut buf, u64::MAX);
        tx.write_all(&buf).await.unwrap();
        assert_eq!(read_varint(&mut rx).await.unwrap(), Some(u64::MAX));
    }
}
