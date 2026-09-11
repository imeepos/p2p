//! pump 半关真实 TCP wire 红绿复核（W-T3 移交 MUST）：W-T3 测试实证「FIN 后
//! 对端写帧，pump 读端 0 字节」（duplex wire 无此象）。本文件以真实 TcpStream
//! 为隧道侧 wire 直接驱动 tunnel_pump 定案：本端写半关（FIN）后，pump 读端
//! 必须仍能收到对端后续帧并落向本地目标（契约 §4 半关闭语义）。

use p2p_protocol::{read_frame, write_frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::{TcpListener, TcpStream};

use crate::pump::tunnel_pump;
use crate::TunnelIo;

const CHUNK: usize = 64 * 1024;

/// 泵级定案探针：local EOF → pump 写帧 + FIN；对端在 FIN 之后补写帧，
/// pump 读端必须照收并落向本地目标，随后对端 FIN 自然收口（error=None）。
#[tokio::test]
async fn pump_read_survives_peer_frames_after_own_write_half_close_real_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    // 先连接后 accept（同任务内顺序完成，杜绝 accept/connect 互等）。
    let mut peer = TcpStream::connect(addr).await.expect("connect");
    let (socket, _) = listener.accept().await.expect("accept");
    let (local_app, pump_io): (DuplexStream, TunnelIo) = {
        let (app, io) = tokio::io::duplex(8192);
        (app, Box::new(io))
    };
    let pump = tokio::spawn(tunnel_pump(Box::new(socket), pump_io, CHUNK, None));

    // 请求方向：local 写完即 EOF → pump 出帧 + 写半关（FIN 上线）。
    let mut local_app = local_app;
    local_app.write_all(b"REQ!").await.unwrap();
    local_app.shutdown().await.unwrap();

    // 对端（raw TcpStream 侧）：读到请求帧（此时本端已/将 FIN），再补写后继帧。
    let request = read_frame(&mut peer).await.expect("request frame");
    assert_eq!(request, b"REQ!");
    write_frame(&mut peer, b"LATE").await.expect("late frame");

    // 定案点：FIN 后 pump 读端必须仍读到对端帧（0 字节/EOF 即半关读缺陷）。
    let mut late = [0u8; 4];
    local_app
        .read_exact(&mut late)
        .await
        .expect("FIN 后 pump 读端必须存活并送达对端后继帧");
    assert_eq!(&late, b"LATE");

    // 对端也收口：pump 两侧自然终了，无错误。
    peer.shutdown().await.unwrap();
    let mut rest = Vec::new();
    local_app.read_to_end(&mut rest).await.unwrap();
    assert!(rest.is_empty(), "对端 finish → 本地读 EOF");
    let result = pump.await.unwrap();
    assert!(
        result.error.is_none(),
        "半关全链必须自然收口：{:?}",
        result.error
    );
    assert_eq!(result.totals.bytes_in, 4, "LATE 帧计入 bytes_in");
    assert_eq!(result.totals.bytes_out, 4, "REQ! 帧计入 bytes_out");
}

/// 对照组（W-T3「读端 0 字节」接线成因留证）：对端整流关闭（读写同关）时，
/// 本端读端收尽在途数据后即 0 字节，属 TCP 正确行为而非 pump 缺陷。
#[tokio::test]
async fn peer_full_close_ends_wire_read_with_zero_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let mut socket = TcpStream::connect(addr).await.expect("connect");
        write_frame(&mut socket, b"DATA").await.unwrap();
        // 整流关闭：drop 即读写同关（对照：写半关是 shutdown(Write) 语义）。
    });
    let (mut wire, _) = listener.accept().await.expect("accept");
    peer.await.unwrap();
    assert_eq!(read_frame(&mut wire).await.unwrap(), b"DATA");
    let err = read_frame(&mut wire)
        .await
        .expect_err("整流关闭后必须 EOF（0 字节），无后继帧");
    assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
}
