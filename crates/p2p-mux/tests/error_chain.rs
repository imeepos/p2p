//! E7-K2 回归：复用层错误必须保留内层错误链（E5 登记项）。
//!
//! E5 登记项：quic_mux transport_err 曾用 to_string 拍平，内层
//! quinn::ConnectionError 的类型与文案全部丢失。本文件断言沿
//! io::Error::source() 可原样还原内层错误。
//! 消融证明：撤掉装箱修复（换回 e.to_string()）后本文件用例转红。

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use p2p_identity::Keypair;
use p2p_transport::{QuicTransport, Transport, TransportAddr};

const WAIT: Duration = Duration::from_secs(5);

fn local(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port)
}

/// 沿 source 链逐层下探，断言 quinn::ConnectionError 可还原且文案与
/// 顶层一致（io::Error 的 Display 委托给载荷，即内层错误原文）。
fn assert_chain_reaches_quinn_connection_error(err: &std::io::Error) {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = cur {
        if let Some(inner) = e.downcast_ref::<quinn::ConnectionError>() {
            assert!(!inner.to_string().is_empty());
            assert_eq!(
                inner.to_string(),
                err.to_string(),
                "io 层文案必须是内层错误原文（不得加工拍平）"
            );
            return;
        }
        cur = e.source();
    }
    panic!("io::Error 的 source 链必须能还原 quinn::ConnectionError，实际链已尽: {err}");
}

#[tokio::test]
async fn open_stream_after_close_keeps_quinn_connection_error() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let server = QuicTransport::bind(local(0), &server_kp)
        .await
        .expect("bind quic");
    let port = server.local_addr().expect("local addr").port();
    let accepted = tokio::spawn(async move { server.accept().await });

    let client = QuicTransport::new().expect("client transport");
    let conn = client
        .dial(
            &TransportAddr::Quic {
                ip: local(port).ip(),
                port,
            },
            &client_kp,
            Some(server_kp.peer_id()),
        )
        .await
        .expect("dial quic");
    let _server_conn = tokio::time::timeout(WAIT, accepted)
        .await
        .expect("server accept in time")
        .expect("server task joins");

    // 本端主动 close 后 open_bi 立即失败：错误必须携带 quinn::ConnectionError
    conn.mux.close();
    let result = tokio::time::timeout(WAIT, conn.mux.open_stream())
        .await
        .expect("open_stream must not hang after close");
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("open_stream must fail after close"),
    };
    assert_chain_reaches_quinn_connection_error(&err);
}
