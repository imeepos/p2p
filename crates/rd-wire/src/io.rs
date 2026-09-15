//! 帧 I/O：把 rd-wire 消息落到 p2p 逻辑流（复用底座帧封装）。
//! 小消息走 varint 长度前缀帧；大载荷（画面/文件数据）走 chunked。

use p2p_protocol::{read_chunked, read_frame, write_chunked, write_frame};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{ControlMsg, FileMsg};

/// 发送控制消息（编码失败即 io error，不落半帧）。
pub async fn send_control<W>(w: &mut W, msg: &ControlMsg) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin + Send,
{
    let bytes = msg.encode().map_err(invalid)?;
    write_frame(w, &bytes).await
}

/// 接收控制消息（非法帧回报 InvalidData）。
pub async fn recv_control<R>(r: &mut R) -> std::io::Result<ControlMsg>
where
    R: AsyncRead + Unpin + Send,
{
    let bytes = read_frame(r).await?;
    ControlMsg::decode(&bytes).map_err(invalid)
}

/// 发送文件消息。
pub async fn send_file<W>(w: &mut W, msg: &FileMsg) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin + Send,
{
    let bytes = msg.encode().map_err(invalid)?;
    write_frame(w, &bytes).await
}

/// 接收文件消息。
pub async fn recv_file<R>(r: &mut R) -> std::io::Result<FileMsg>
where
    R: AsyncRead + Unpin + Send,
{
    let bytes = read_frame(r).await?;
    FileMsg::decode(&bytes).map_err(invalid)
}

/// 大载荷写出（≤ CHUNK_DATA_SIZE 单片，其余分块；≤ 64 MiB 重组上限由底座约束）。
pub async fn send_large<W>(w: &mut W, payload: &[u8]) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin + Send,
{
    write_chunked(w, payload).await
}

/// 大载荷读入。
pub async fn recv_large<R>(r: &mut R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin + Send,
{
    read_chunked(r).await
}

fn invalid<E>(e: E) -> std::io::Error
where
    E: std::fmt::Display,
{
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Role;
    use tokio::io::duplex;

    #[tokio::test]
    async fn control_io_roundtrip() {
        let (mut a, mut b) = duplex(4096);
        let msg = ControlMsg::Heartbeat { seq: 42 };
        let (w, r) = tokio::join!(send_control(&mut a, &msg), recv_control(&mut b));
        w.unwrap();
        assert_eq!(r.unwrap(), msg);
    }

    #[tokio::test]
    async fn large_payload_roundtrip() {
        let (mut a, mut b) = duplex(64 * 1024);
        let payload: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let (w, r) = tokio::join!(send_large(&mut a, &payload), recv_large(&mut b));
        w.unwrap();
        assert_eq!(r.unwrap(), payload);
    }

    #[tokio::test]
    async fn invalid_control_rejected() {
        let (mut a, mut b) = duplex(4096);
        let bad = br#"{"type":"hello","v":99,"role":"viewer","session_id":"0123456789abcdef","caps":{"audio":false,"file":false,"clipboard":false}}"#;
        let (w, r) = tokio::join!(p2p_protocol::write_frame(&mut a, bad), recv_control(&mut b));
        w.unwrap();
        let err = r.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn role_serde_names() {
        assert_eq!(serde_json::to_string(&Role::Viewer).unwrap(), "\"viewer\"");
    }
}
