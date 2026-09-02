//! 半开握手超时验收（安全审查 1 期 M3）：慢对端必须在 deadline 内被断开，
//! 且错误信息明确指向握手超时，而非静默挂起。

use std::time::{Duration, Instant};

use p2p_identity::Keypair;
use p2p_security::{NoiseXx, SecurityUpgrade};

/// 覆盖用的短超时：测试要求远小于默认 10s
const SHORT_TIMEOUT: Duration = Duration::from_millis(200);

#[tokio::test]
async fn silent_peer_times_out_with_deadline_error() {
    let alice = Keypair::generate();
    let (a, b) = tokio::io::duplex(64 * 1024);
    // 对端持有连接但永不写数据：模拟 slowloris 半开握手
    let held = tokio::spawn(async move {
        let _keep_open = b;
        std::future::pending::<()>().await;
    });

    let started = Instant::now();
    let result = NoiseXx::new()
        .with_handshake_timeout(SHORT_TIMEOUT)
        .outbound(Box::new(a), &alice, None)
        .await;
    let elapsed = started.elapsed();

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("silent peer must not complete handshake"),
    };
    assert!(
        err.to_string().contains("handshake deadline exceeded"),
        "error must name the deadline, got: {err}"
    );
    assert!(
        elapsed >= SHORT_TIMEOUT,
        "must wait at least the configured timeout"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "must not fall back to the default timeout, took {elapsed:?}"
    );
    held.abort();
}

#[tokio::test]
async fn half_sent_frame_times_out_with_deadline_error() {
    let alice = Keypair::generate();
    let (a, mut slow) = tokio::io::duplex(64 * 1024);
    // 对端只写 2 字节帧长（声明 100 字节帧体）后挂起：半开帧
    let slow_task = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let _ = slow.write_all(&100u16.to_be_bytes()).await;
        let _ = slow.flush().await;
        std::future::pending::<()>().await;
    });

    let result = NoiseXx::new()
        .with_handshake_timeout(SHORT_TIMEOUT)
        .outbound(Box::new(a), &alice, None)
        .await;

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("half-sent frame must not complete handshake"),
    };
    assert!(
        err.to_string().contains("handshake deadline exceeded"),
        "error must name the deadline, got: {err}"
    );
    slow_task.abort();
}

#[tokio::test]
async fn custom_timeout_still_allows_normal_handshake() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let (a, b) = tokio::io::duplex(64 * 1024);

    let client = NoiseXx::new().with_handshake_timeout(Duration::from_secs(2));
    let server = NoiseXx::new().with_handshake_timeout(Duration::from_secs(2));
    let (server_res, client_res) = tokio::join!(
        server.inbound(Box::new(b), &bob),
        client.outbound(Box::new(a), &alice, Some(bob.peer_id()))
    );
    let (client_peer, _) = client_res.expect("client handshake with custom timeout");
    let (server_peer, _) = server_res.expect("server handshake with custom timeout");
    assert_eq!(client_peer, bob.peer_id());
    assert_eq!(server_peer, alice.peer_id());
}
