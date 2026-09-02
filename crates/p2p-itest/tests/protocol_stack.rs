//! 互操作：identity 推导 PeerId + Noise 安全层 + protocol 帧/chunked
//! 在一条 duplex 字节流上贯通，2MiB payload 收发一致。

use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{
    flatten_io, open_with_protocol, read_chunked, read_frame, read_protocol_id, write_chunked,
    write_frame, ProtocolError, ProtocolId, MAX_FRAME_SIZE,
};
use p2p_security::{NoiseXx, SecurityUpgrade};
use tokio::io::AsyncWriteExt;

use p2p_itest::expect_within;

const MIB: usize = 1024 * 1024;
const LIMIT: Duration = Duration::from_secs(30);

/// 服务侧：Noise inbound → 协议 ID 校验 → chunked 收 → 原样回写。返回收到的 payload。
async fn echo_server(
    stream: BoxedStream,
    kp: Keypair,
    expect_client: PeerId,
    proto: ProtocolId,
) -> Vec<u8> {
    let (peer, mut secured) = NoiseXx::new()
        .inbound(stream, &kp)
        .await
        .expect("server handshake must succeed");
    assert_eq!(
        peer, expect_client,
        "server must derive caller identity from key material"
    );

    let id = read_protocol_id(&mut secured).await.expect("protocol id");
    assert_eq!(id, proto, "routed protocol id must match");

    let payload = read_chunked(&mut secured).await.expect("chunked read");
    write_chunked(&mut secured, &payload)
        .await
        .expect("chunked echo");
    secured.flush().await.expect("flush echo");
    payload
}

#[tokio::test]
async fn secured_stack_roundtrips_2mib_chunked() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (a, b) = tokio::io::duplex(256 * 1024);
    let proto = ProtocolId::new("/itest/echo/1").expect("valid protocol id");

    let server = tokio::spawn(echo_server(
        Box::new(b),
        bob.clone(),
        alice.peer_id(),
        proto.clone(),
    ));

    let client = NoiseXx::new();
    let (server_peer, secured) = expect_within(
        "client handshake",
        client.outbound(Box::new(a), &alice, Some(bob.peer_id())),
        LIMIT,
    )
    .await
    .expect("client handshake must succeed");
    assert_eq!(
        server_peer,
        bob.peer_id(),
        "client must derive server identity"
    );

    let stream = open_with_protocol(secured, &proto)
        .await
        .expect("protocol handshake");
    let (mut rh, mut wh) = tokio::io::split(stream);

    let payload: Vec<u8> = (0..2 * MIB).map(|i| (i % 253) as u8).collect();
    let echo = expect_within(
        "2MiB chunked echo",
        async {
            let writer = async {
                write_chunked(&mut wh, &payload)
                    .await
                    .expect("write chunked");
                wh.flush().await.expect("flush chunked");
            };
            let (_, r) = tokio::join!(writer, read_chunked(&mut rh));
            r.expect("read chunked")
        },
        LIMIT,
    )
    .await;
    assert_eq!(echo, payload, "2MiB payload must roundtrip byte-identical");

    let served = server.await.expect("server task");
    assert_eq!(served, payload, "server received exactly what was sent");
}

#[tokio::test]
async fn protocol_frames_pass_through_noise_secured_stream() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (a, b) = tokio::io::duplex(64 * 1024);

    let bob_id = bob.peer_id();
    let server = tokio::spawn(async move {
        let (_, mut secured) = NoiseXx::new()
            .inbound(Box::new(b), &bob)
            .await
            .expect("server handshake");
        read_frame(&mut secured).await.expect("server read frame")
    });

    let (_, secured) = NoiseXx::new()
        .outbound(Box::new(a), &alice, Some(bob_id))
        .await
        .expect("client handshake");
    let mut secured = secured;
    write_frame(&mut secured, b"over-noise")
        .await
        .expect("write frame");
    secured.flush().await.expect("flush frame");

    let got = server.await.expect("server task");
    assert_eq!(
        got,
        b"over-noise".to_vec(),
        "frames must survive the security layer"
    );
}

#[tokio::test]
async fn max_frame_roundtrips_and_oversize_rejected() {
    // 边界值：恰好 1MiB 的帧必须原样通过
    let (mut tx, mut rx) = tokio::io::duplex(2 * MIB);
    let payload = vec![0xa5u8; MAX_FRAME_SIZE as usize];
    let (w, r) = tokio::join!(write_frame(&mut tx, &payload), read_frame(&mut rx));
    w.expect("write max frame");
    assert_eq!(
        r.expect("read max frame"),
        payload,
        "boundary frame must be intact"
    );

    // 超上限长度前缀：读端必须显式 FrameTooLarge，不得静默截断
    let (mut tx2, mut rx2) = tokio::io::duplex(64);
    tx2.write_all(&varint(u64::from(MAX_FRAME_SIZE) + 1))
        .await
        .expect("raw write");
    let err = read_frame(&mut rx2)
        .await
        .expect_err("oversize must surface as error");
    match flatten_io(err) {
        ProtocolError::FrameTooLarge(n) => assert_eq!(n, u64::from(MAX_FRAME_SIZE) + 1),
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// 协议 ID 握手后的首个载荷帧：开流侧写协议 ID + 帧必须被收流侧原样解析。
#[tokio::test]
async fn protocol_id_handshake_then_single_frame_over_duplex() {
    let (tx, rx) = tokio::io::duplex(1024);
    let proto = ProtocolId::new("/itest/handshake/1").expect("valid id");

    let server = tokio::spawn(async move {
        let mut boxed: BoxedStream = Box::new(rx);
        let id = read_protocol_id(&mut boxed)
            .await
            .expect("read protocol id");
        let body = read_frame(&mut boxed).await.expect("read frame");
        (id, body)
    });

    let opened: BoxedStream = Box::new(tx);
    let mut opened = open_with_protocol(opened, &proto)
        .await
        .expect("protocol handshake");
    write_frame(&mut opened, b"payload")
        .await
        .expect("write payload");

    let (id, body) = server.await.expect("server task");
    assert_eq!(id, proto);
    assert_eq!(body, b"payload".to_vec());
}
