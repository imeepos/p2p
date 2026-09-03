//! 双栈拨号端点验收（2026-09-04 线上事故回归）：
//! [::]:0 双栈端点对 V4（v4-mapped 出网）与 V6 目标都必须可拨。
//! 事故形态：拨号端点单绑 0.0.0.0 时，地址簿全部 IPv6 候选本地即拒
//! （invalid remote address），直连路径整体坏死，流量被迫全压 relay。

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use p2p_identity::{Keypair, PeerId};
use p2p_transport::{QuicTransport, SecureConn, Transport, TransportAddr, TransportError};

fn v4_addr(port: u16) -> TransportAddr {
    TransportAddr::Quic {
        ip: IpAddr::from([127, 0, 0, 1]),
        port,
    }
}

fn v6_addr(port: u16) -> TransportAddr {
    TransportAddr::Quic {
        ip: IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        port,
    }
}

/// 回显服务端：逐连接逐流双向拷贝，返回监听端口。
async fn spawn_echo(kp: &Keypair, bind: SocketAddr) -> u16 {
    let server = Arc::new(QuicTransport::bind(bind, kp).await.expect("bind quic"));
    let port = server.local_addr().expect("local addr").port();
    tokio::spawn(async move {
        while let Some(conn) = server.accept().await {
            tokio::spawn(async move { echo_conn(conn).await });
        }
    });
    port
}

async fn echo_conn(conn: SecureConn) {
    while let Some(stream) = conn.mux.accept_stream().await {
        let (mut rx, mut tx) = tokio::io::split(stream);
        tokio::spawn(async move {
            let _ = tokio::io::copy(&mut rx, &mut tx).await;
        });
    }
}

/// 断言拨号后一个来回：写 shutdown，读回等长等内容回显。
async fn assert_roundtrip(
    client: &QuicTransport,
    client_kp: &Keypair,
    server_peer: PeerId,
    addr: &TransportAddr,
    tag: &str,
) {
    let conn = client
        .dial(addr, client_kp, Some(server_peer))
        .await
        .unwrap_or_else(|e| panic!("{tag}: dial must succeed, got {e}"));
    assert_eq!(conn.remote, server_peer, "{tag}: identity mismatch");
    let mut stream = conn.mux.open_stream().await.expect("open stream");
    let payload = b"dualstack-probe".to_vec();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(&payload).await.expect("write");
    stream.shutdown().await.expect("shutdown");
    let mut echoed = Vec::new();
    stream.read_to_end(&mut echoed).await.expect("read");
    assert_eq!(echoed, payload, "{tag}: echo mismatch");
}

#[tokio::test]
async fn dual_stack_endpoint_dials_v4_via_mapped() {
    let server_kp = Keypair::generate();
    let port = spawn_echo(&server_kp, SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0)).await;
    let client = QuicTransport::new().expect("client transport");
    assert_roundtrip(
        &client,
        &Keypair::generate(),
        server_kp.peer_id(),
        &v4_addr(port),
        "v4-mapped dial",
    )
    .await;
}

#[tokio::test]
async fn unspecified_target_rejected_fast() {
    // 未指定地址必须本地即拒（契约性 Dial 变体）：映射成 v4-mapped 后
    // is_unspecified 失真，会绕过 quinn-proto 确定性拒绝退化为吃满握手超时。
    let client = QuicTransport::new().expect("client transport");
    let addr = TransportAddr::Quic {
        ip: IpAddr::from([0, 0, 0, 0]),
        port: 1,
    };
    let started = std::time::Instant::now();
    // SecureConn 无 Debug：expect_err 不可用（known-issues 2026-09-03），match 取 Err 臂
    let outcome = match client.dial(&addr, &Keypair::generate(), None).await {
        Err(e) => e,
        Ok(_) => panic!("unspecified target must be rejected"),
    };
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "must fail fast"
    );
    match outcome {
        TransportError::Dial { reason, .. } => {
            assert!(reason.contains("unspecified"), "reason: {reason}");
        }
        other => panic!("expected Dial text variant, got {other:?}"),
    }
}

#[tokio::test]
async fn dual_stack_endpoint_dials_v6() {
    let server_kp = Keypair::generate();
    let port = spawn_echo(
        &server_kp,
        SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 0),
    )
    .await;
    let client = QuicTransport::new().expect("client transport");
    assert_roundtrip(
        &client,
        &Keypair::generate(),
        server_kp.peer_id(),
        &v6_addr(port),
        "v6 dial",
    )
    .await;
}
