//! 双向字节哑泵（契约 §4）：出站载荷 ≤ chunk_size 分帧；读侧容忍任意 ≤1 MiB
//! 帧界（read_frame 重组）。finish = 本端写半关（shutdown → 对端读 EOF），
//! 两方向独立流动；任一真实 IO 错误/超时 → 整隧道关闭。

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p_protocol::{read_frame, write_frame};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::task::JoinHandle;

/// 字节计数：bytes_in = 自隧道收得并落向本地目标；bytes_out = 自本地目标收得
/// 并送向隧道（被访侧视角，契约 §5 同义）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PumpTotals {
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// 泵终态：error=None 即双向自然半关收口；Some 即整隧道关闭原因。
pub struct PumpResult {
    pub totals: PumpTotals,
    pub error: Option<io::Error>,
}

/// 泵至双向终了。stream = 隧道侧（帧封装），io = 本地目标侧（裸字节）；
/// 返回即两侧均已收口/关闭。
pub async fn tunnel_pump<S, I>(
    stream: S,
    io: I,
    chunk_size: usize,
    session_timeout: Option<Duration>,
) -> PumpResult
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let bytes_in = Arc::new(AtomicU64::new(0));
    let bytes_out = Arc::new(AtomicU64::new(0));
    let (stream_r, stream_w) = tokio::io::split(stream);
    let (io_r, io_w) = tokio::io::split(io);
    let mut to_local = tokio::spawn(copy_frames_to_io(stream_r, io_w, bytes_in.clone()));
    let mut to_peer = tokio::spawn(copy_io_to_frames(
        io_r,
        stream_w,
        chunk_size,
        bytes_out.clone(),
    ));
    let error = wait_both(&mut to_local, &mut to_peer, session_timeout).await;
    PumpResult {
        totals: PumpTotals {
            bytes_in: bytes_in.load(Ordering::Relaxed),
            bytes_out: bytes_out.load(Ordering::Relaxed),
        },
        error,
    }
}

/// 第一条方向真实出错或会话超时即整隧道关闭（中止另一方向并收尸）；自然半关
/// 则等两方向都收口。JoinHandle 完成后不可再 poll：每个句柄只经由本函数收口。
async fn wait_both(
    to_local: &mut JoinHandle<io::Result<()>>,
    to_peer: &mut JoinHandle<io::Result<()>>,
    session_timeout: Option<Duration>,
) -> Option<io::Error> {
    let (mut local_done, mut peer_done) = (false, false);
    let deadline = session_timeout.map(|t| tokio::time::Instant::now() + t);
    loop {
        if local_done && peer_done {
            return None;
        }
        let joined = tokio::select! {
            _ = async {
                match deadline {
                    Some(d) => tokio::time::sleep_until(d).await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                reap(to_local, to_peer, local_done, peer_done).await;
                return Some(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "tunnel session timeout",
                ));
            }
            joined = &mut *to_local, if !local_done => {
                local_done = true;
                join_error(joined, "tunnel->local")
            }
            joined = &mut *to_peer, if !peer_done => {
                peer_done = true;
                join_error(joined, "local->tunnel")
            }
        };
        if let Some(error) = joined {
            reap(to_local, to_peer, local_done, peer_done).await;
            return Some(error);
        }
    }
}

/// 收割被中止（未完成）的句柄，让方向任务落地释放流；已完成的句柄不可再 poll。
async fn reap(
    to_local: &mut JoinHandle<io::Result<()>>,
    to_peer: &mut JoinHandle<io::Result<()>>,
    local_done: bool,
    peer_done: bool,
) {
    to_local.abort();
    to_peer.abort();
    if !local_done {
        let _ = to_local.await;
    }
    if !peer_done {
        let _ = to_peer.await;
    }
}

fn join_error(
    joined: Result<io::Result<()>, tokio::task::JoinError>,
    context: &'static str,
) -> Option<io::Error> {
    match joined {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(cancelled) => Some(io::Error::other(format!("{context}: {cancelled}"))),
    }
}

/// 隧道 → 本地目标：帧重组为连续字节流（容忍任意 ≤1 MiB 帧界）。对端 finish
/// （读 EOF）即本地写半关，方向自然终了（契约 §4）。
async fn copy_frames_to_io<R, W>(mut src: R, mut dst: W, counter: Arc<AtomicU64>) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    loop {
        let frame = match read_frame(&mut src).await {
            Ok(frame) => frame,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                dst.shutdown().await?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if frame.is_empty() {
            continue;
        }
        counter.fetch_add(frame.len() as u64, Ordering::Relaxed);
        dst.write_all(&frame).await?;
        dst.flush().await?;
    }
}

/// 本地目标 → 隧道：单次读 ≤ chunk_size 即一帧载荷上限（契约 §1）。本地 EOF
/// 即对端写半关（shutdown → 对端读 EOF），方向自然终了。
async fn copy_io_to_frames<R, W>(
    mut src: R,
    mut dst: W,
    chunk_size: usize,
    counter: Arc<AtomicU64>,
) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    let mut buf = vec![0u8; chunk_size];
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            dst.shutdown().await?;
            return Ok(());
        }
        counter.fetch_add(n as u64, Ordering::Relaxed);
        write_frame(&mut dst, &buf[..n]).await?;
        dst.flush().await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TunnelIo;
    use p2p_mux::BoxedStream;
    use p2p_protocol::MAX_FRAME_SIZE;
    use tokio::io::duplex;

    const CHUNK: usize = 64 * 1024;

    #[tokio::test]
    async fn outbound_chunks_are_capped_and_merged_in_order() {
        let payload: Vec<u8> = (0..CHUNK * 2 + 17).map(|i| (i % 251) as u8).collect();
        let (mut peer_end, stream_end) = duplex(CHUNK * 2 + 4096);
        let (_stream_r, stream_w) = tokio::io::split(stream_end);
        let src = std::io::Cursor::new(payload.clone());
        let task = tokio::spawn(copy_io_to_frames(
            src,
            stream_w,
            CHUNK,
            Arc::new(AtomicU64::new(0)),
        ));
        let mut merged = Vec::new();
        let (mut frames, mut max_payload) = (0usize, 0usize);
        while let Ok(frame) = read_frame(&mut peer_end).await {
            frames += 1;
            max_payload = max_payload.max(frame.len());
            merged.extend_from_slice(&frame);
        }
        task.await.unwrap().unwrap();
        assert_eq!(merged, payload, "帧序重组 = 原字节流（合并语义）");
        assert!(max_payload <= CHUNK, "出站载荷 {max_payload} 必须 ≤ {CHUNK}");
        assert!(frames >= 3, "1.5 MiB 必然分多帧: {frames}");
    }

    #[tokio::test]
    async fn inbound_frames_of_any_boundary_merge_contiguously() {
        let mut payload = b"GET / HTTP/1.1\r\n\r\nBODY".to_vec();
        payload.extend_from_slice(&vec![7u8; MAX_FRAME_SIZE as usize]);
        let (mut peer_end, stream_end) = duplex(4096);
        let (stream_r, _stream_w) = tokio::io::split(stream_end);
        let (mut local_test, local_rsp) = duplex(4096);
        let (_io_r, io_w) = tokio::io::split(local_rsp);
        let task = tokio::spawn(copy_frames_to_io(
            stream_r,
            io_w,
            Arc::new(AtomicU64::new(0)),
        ));
        let expected = payload.clone();
        let push = tokio::spawn(async move {
            write_frame(&mut peer_end, &payload[..5]).await.unwrap();
            write_frame(&mut peer_end, &payload[5..9]).await.unwrap();
            write_frame(&mut peer_end, &payload[9..22]).await.unwrap();
            write_frame(&mut peer_end, &payload[22..]).await.unwrap();
        });
        let mut merged = Vec::new();
        local_test.read_to_end(&mut merged).await.unwrap();
        push.await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(merged, expected, "异形帧界（含 1 MiB 满帧）合并为连续字节流");
    }

    #[tokio::test]
    async fn full_pump_roundtrip_counts_bytes_and_closes_cleanly() {
        let (mut visitor, stream_end) = duplex(8192);
        let (mut local_test, local_rsp) = duplex(8192);
        let stream: BoxedStream = Box::new(stream_end);
        let io: TunnelIo = Box::new(local_rsp);
        let task = tokio::spawn(tunnel_pump(stream, io, CHUNK, None));
        // 隧道侧（visitor）按帧收发；本地目标侧（local_test）裸字节。
        write_frame(&mut visitor, b"request").await.unwrap();
        let mut buf = [0u8; 7];
        local_test.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"request");
        local_test.write_all(b"response").await.unwrap();
        let frame = read_frame(&mut visitor).await.unwrap();
        assert_eq!(frame, b"response");
        visitor.shutdown().await.unwrap();
        let mut rest = Vec::new();
        local_test.read_to_end(&mut rest).await.unwrap();
        assert!(rest.is_empty(), "对端 finish → 本地读 EOF");
        local_test.shutdown().await.unwrap();
        let result = task.await.unwrap();
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(
            result.totals,
            PumpTotals {
                bytes_in: 7,
                bytes_out: 8
            }
        );
    }
}
