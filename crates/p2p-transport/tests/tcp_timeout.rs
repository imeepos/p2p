//! TCP 半开握手超时验收（安全审查 1 期 M3）：
//! 对端 accept 后不说话，拨号端的 Noise 握手必须在配置时限内失败且报因明确。

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use p2p_identity::Keypair;
use p2p_transport::{TcpTransport, Transport, TransportAddr};

#[tokio::test]
async fn half_open_tcp_handshake_times_out_with_clear_error() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let server = TcpTransport::new();
    let listener = server
        .bind(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
        .await
        .expect("bind tcp");
    let port = listener.local_addr().expect("local addr").port();

    // 服务端 accept 到连接后不做任何 Noise 应答，只持有流
    let hold_task = tokio::spawn(async move {
        let (_held, _peer) = listener.accept().await.expect("accept");
        std::future::pending::<()>().await;
    });

    let client = TcpTransport::with_timeouts(Duration::from_secs(5), Duration::from_millis(200));
    let addr = TransportAddr::Tcp {
        ip: IpAddr::from([127, 0, 0, 1]),
        port,
    };
    let started = std::time::Instant::now();
    let result = client
        .dial(&addr, &client_kp, Some(server_kp.peer_id()))
        .await;
    let elapsed = started.elapsed();

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("half-open handshake must fail"),
    };
    // E7-K2 错误链保真：安全层错误挂在 source 链，内层 SecurityError
    // 类型与文案必须可还原（不再 to_string 拍平）
    let source = match &err {
        p2p_transport::TransportError::HandshakeChained { source } => source,
        other => panic!("expected HandshakeChained, got: {other:?}"),
    };
    let security = source
        .downcast_ref::<p2p_security::SecurityError>()
        .expect("source must downcast to SecurityError");
    let reason = match security {
        p2p_security::SecurityError::Handshake(text) => text.clone(),
        other => panic!("expected SecurityError::Handshake, got: {other:?}"),
    };
    assert!(
        reason.contains("handshake deadline exceeded"),
        "error must name the deadline, got: {reason}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "handshake timeout must apply, took {elapsed:?}"
    );
    hold_task.abort();
}
