//! 互操作：p2p-identity 密钥 ↔ p2p-security Noise XX 在 duplex 字节流上
//! 的握手互认与篡改失败路径。失败必须显式报错，不允许静默通过。

use std::time::Duration;

use p2p_identity::Keypair;
use p2p_mux::BoxedStream;
use p2p_security::{NoiseXx, SecurityError, SecurityUpgrade};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

use p2p_itest::expect_within;

const BUF: usize = 64 * 1024;
const LIMIT: Duration = Duration::from_secs(5);

fn duplex_pair() -> (BoxedStream, BoxedStream) {
    let (a, b) = tokio::io::duplex(BUF);
    (Box::new(a), Box::new(b))
}

#[tokio::test]
async fn xx_handshake_derives_counterparty_peer_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (a, b) = duplex_pair();

    let (server, client) = tokio::join!(
        expect_within("server handshake", NoiseXx.inbound(b, &bob), LIMIT),
        expect_within(
            "client handshake",
            NoiseXx.outbound(a, &alice, Some(bob.peer_id())),
            LIMIT
        )
    );
    let (server_peer, mut server_stream) = server.expect("server handshake must succeed");
    let (client_peer, mut client_stream) = client.expect("client handshake must succeed");

    // 双方推导出的 PeerId 必须来自对端密钥材料，而非任何自报字段
    assert_eq!(server_peer, alice.peer_id(), "server side derivation");
    assert_eq!(client_peer, bob.peer_id(), "client side derivation");

    client_stream.write_all(b"identity-probe").await.expect("write");
    client_stream.flush().await.expect("flush");
    let mut got = [0u8; 14];
    server_stream.read_exact(&mut got).await.expect("read");
    assert_eq!(&got, b"identity-probe", "secured stream must carry bytes intact");
}

#[tokio::test]
async fn seed_restored_keypair_keeps_peer_id() {
    let kp = Keypair::from_seed(&[9u8; 32]);
    let restored = Keypair::from_seed(&kp.to_seed_bytes());
    assert_eq!(kp.peer_id(), restored.peer_id(), "identity must survive seed roundtrip");
}

/// 单向搬运 [u16 帧长][帧体]；第 tamper_frame 帧翻转一个帧体字节后继续转发。
async fn relay_direction(
    mut read: impl AsyncRead + Unpin,
    mut write: impl AsyncWrite + Unpin,
    tamper_frame: usize,
) {
    let mut index = 0usize;
    loop {
        let mut len_buf = [0u8; 2];
        if read.read_exact(&mut len_buf).await.is_err() {
            return;
        }
        let len = u16::from_be_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        if read.read_exact(&mut body).await.is_err() {
            return;
        }
        if index == tamper_frame && len > 0 {
            body[0] ^= 0xff;
        }
        index += 1;
        if write.write_all(&len_buf).await.is_err() || write.write_all(&body).await.is_err() {
            return;
        }
        if write.flush().await.is_err() {
            return;
        }
    }
}

#[tokio::test]
async fn tampered_first_handshake_frame_fails_both_sides() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (initiator, mitm_in) = tokio::io::duplex(BUF);
    let (mitm_out, responder) = tokio::io::duplex(BUF);
    let (in_r, in_w) = tokio::io::split(mitm_in);
    let (out_r, out_w) = tokio::io::split(mitm_out);
    // split 是 BiLock：只有整条 duplex 两半全部归还，对端才能看到 EOF。
    // 正向管道退出即通知反向管道退出，避免响应端永久等帧。
    let (done_tx, done_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        relay_direction(in_r, out_w, 0).await;
        let _ = done_tx.send(());
    });
    tokio::spawn(async move {
        let relayed = relay_direction(out_r, in_w, usize::MAX);
        tokio::select! {
            () = relayed => {}
            _ = done_rx => {}
        }
    });

    let (server, client) = tokio::join!(
        NoiseXx.inbound(Box::new(responder), &bob),
        NoiseXx.outbound(Box::new(initiator), &alice, Some(bob.peer_id()))
    );
    let server_err = server.err().expect("responder must fail on tampered msg1");
    let client_err = client.err().expect("initiator must fail when responder aborts");
    for err in [server_err, client_err] {
        assert!(
            matches!(err, SecurityError::Handshake(_) | SecurityError::IdentityUnverified),
            "expected explicit handshake failure, got {err:?}"
        );
    }
}

#[tokio::test]
async fn wrong_expected_peer_is_explicit_peer_mismatch() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let eve = Keypair::generate();
    let (a, b) = duplex_pair();

    let (server, client) = tokio::join!(
        NoiseXx.inbound(b, &bob),
        NoiseXx.outbound(a, &alice, Some(eve.peer_id()))
    );
    match client {
        Err(SecurityError::PeerMismatch { expected, actual }) => {
            assert_eq!(expected, eve.peer_id().to_string());
            assert_eq!(actual, bob.peer_id().to_string());
        }
        Err(other) => panic!("initiator must report PeerMismatch, got {other:?}"),
        Ok(_) => panic!("initiator must not accept an unexpected peer"),
    }
    assert!(server.is_err(), "responder must not complete a hijacked dial either");
}
