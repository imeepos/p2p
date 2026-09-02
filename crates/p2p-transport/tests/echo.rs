//! 同机端到端验收（coordination.md K 包）：
//! QUIC 直连与 TCP+Noise 各跑一遍 互拨 -> 断言对端 PeerId -> 64KB 字节 echo。

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use p2p_identity::{Keypair, PeerId};
use p2p_transport::{
    QuicTransport, SecureConn, TcpTransport, Transport, TransportAddr, TransportError,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const PAYLOAD_LEN: usize = 64 * 1024;

fn local_addr(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port)
}

fn test_payload() -> Vec<u8> {
    (0..PAYLOAD_LEN).map(|i| (i % 251) as u8).collect()
}

/// 回显服务：上报观察到的对端身份，循环收流双向拷贝直至连接关闭。
async fn echo_until_closed(conn: SecureConn, seen: mpsc::Sender<PeerId>) {
    let _ = seen.send(conn.remote).await;
    while let Some(stream) = conn.mux.accept_stream().await {
        let (mut rx, mut tx) = tokio::io::split(stream);
        match tokio::io::copy(&mut rx, &mut tx).await {
            Ok(_) => {
                if let Err(e) = tx.shutdown().await {
                    eprintln!("echo stream shutdown error: {e}");
                }
            }
            Err(e) => {
                eprintln!("echo stream error: {e}");
                break;
            }
        }
    }
}

/// 拨号端：断言身份、开流写 64KB、半关写端、读回完整回显。
async fn drive_echo(conn: SecureConn, server_peer: PeerId) {
    assert_eq!(conn.remote, server_peer, "dialer must see server identity");
    let mut stream = conn.mux.open_stream().await.expect("open stream");
    let payload = test_payload();
    stream.write_all(&payload).await.expect("write payload");
    stream.shutdown().await.expect("shutdown write half");
    let mut echoed = Vec::new();
    stream.read_to_end(&mut echoed).await.expect("read echo");
    assert_eq!(echoed.len(), PAYLOAD_LEN, "echo must be complete");
    assert_eq!(echoed, payload, "echo must match byte for byte");
}

#[tokio::test]
async fn quic_echo_with_peer_id_and_64k_stream() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let server = Arc::new(
        QuicTransport::bind(local_addr(0), &server_kp)
            .await
            .expect("bind quic"),
    );
    let port = server.local_addr().expect("local addr").port();

    let (seen_tx, mut seen_rx) = mpsc::channel(1);
    let srv = Arc::clone(&server);
    let accept_task = tokio::spawn(async move {
        let conn = srv.accept().await.expect("server accept");
        echo_until_closed(conn, seen_tx).await;
    });

    let client = QuicTransport::new().expect("client transport");
    let addr = TransportAddr::Quic { ip: local_addr(port).ip(), port };
    let conn = client
        .dial(&addr, &client_kp, Some(server_kp.peer_id()))
        .await
        .expect("quic dial");
    drive_echo(conn, server_kp.peer_id()).await;

    accept_task.await.expect("server task");
    let seen = seen_rx.recv().await.expect("server observed client");
    assert_eq!(
        seen, client_kp.peer_id(),
        "server must derive client identity from certificate"
    );
}

#[tokio::test]
async fn tcp_noise_echo_with_peer_id_and_64k_stream() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let server = TcpTransport::new();
    let listener = server
        .bind(local_addr(0))
        .await
        .expect("bind tcp");
    let port = listener.local_addr().expect("local addr").port();

    let (seen_tx, mut seen_rx) = mpsc::channel(1);
    let server_kp_in_task = server_kp.clone();
    let accept_task = tokio::spawn(async move {
        let conn = server
            .accept(&listener, &server_kp_in_task)
            .await
            .expect("server accept");
        echo_until_closed(conn, seen_tx).await;
    });

    let client = TcpTransport::new();
    let addr = TransportAddr::Tcp { ip: local_addr(port).ip(), port };
    let conn = client
        .dial(&addr, &client_kp, Some(server_kp.peer_id()))
        .await
        .expect("tcp dial");
    drive_echo(conn, server_kp.peer_id()).await;

    accept_task.await.expect("server task");
    let seen = seen_rx.recv().await.expect("server observed client");
    assert_eq!(
        seen, client_kp.peer_id(),
        "server must derive client identity from noise payload"
    );
}

#[tokio::test]
async fn quic_dial_rejects_wrong_expected_peer() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let eve_kp = Keypair::generate();
    let server = Arc::new(
        QuicTransport::bind(local_addr(0), &server_kp)
            .await
            .expect("bind quic"),
    );
    let port = server.local_addr().expect("local addr").port();
    let srv = Arc::clone(&server);
    let accept_task = tokio::spawn(async move {
        // 服务端握手本身成立，连接会被产出后被丢弃
        let _ = srv.accept().await;
    });

    let client = QuicTransport::new().expect("client transport");
    let addr = TransportAddr::Quic { ip: local_addr(port).ip(), port };
    let outcome = client
        .dial(&addr, &client_kp, Some(eve_kp.peer_id()))
        .await;
    match outcome {
        Err(TransportError::PeerMismatch { expected, actual }) => {
            assert_eq!(expected, eve_kp.peer_id().to_string());
            assert_eq!(actual, server_kp.peer_id().to_string());
        }
        other => panic!(
            "expected PeerMismatch, got {}",
            match other {
                Err(e) => format!("err: {e}"),
                Ok(_) => "Ok(SecureConn)".to_string(),
            }
        ),
    }
    accept_task.await.expect("server task");
}