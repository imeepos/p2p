//! 回归（S 装配实测）：close() 必须主动断开存量 QUIC 连接，
//! 对端可见关闭、本地任务不悬挂。

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::Keypair;
use p2p_transport::{QuicTransport, Transport, TransportAddr};

const WAIT: Duration = Duration::from_secs(5);

fn local(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port)
}

#[tokio::test]
async fn close_notifies_peer_and_does_not_hang() {
    let server_kp = Keypair::generate();
    let client_kp = Keypair::generate();
    let server = Arc::new(
        QuicTransport::bind(local(0), &server_kp)
            .await
            .expect("bind quic"),
    );
    let port = server.local_addr().expect("local addr").port();

    // 服务端 accept 一条连接，建立完成信号回传后才允许 close
    let srv = Arc::clone(&server);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
    let accepted = tokio::spawn(async move {
        let conn = srv.accept().await.expect("server accept");
        let _ = ready_tx.send(());
        tokio::time::timeout(WAIT, async {
            while conn.mux.accept_stream().await.is_some() {}
        })
        .await
        .expect("server connection must close after endpoint close");
    });

    let client = QuicTransport::new().expect("client transport");
    let addr = TransportAddr::Quic {
        ip: local(port).ip(),
        port,
    };
    let conn = client
        .dial(&addr, &client_kp, Some(server_kp.peer_id()))
        .await
        .expect("dial");

    // 等服务端确认已 accept（握手在两端完成），避免 close 取消 pending incoming
    tokio::time::timeout(WAIT, ready_rx)
        .await
        .expect("server accept ready in time")
        .ok();

    server.close();

    // 客户端可见对端关闭：流接收在有限时间内结束（None）
    let peer_closed = tokio::time::timeout(WAIT, async {
        while conn.mux.accept_stream().await.is_some() {}
    })
    .await;
    assert!(
        peer_closed.is_ok(),
        "client must observe connection closure after close()"
    );

    // 本地不悬挂：close 后 accept 返回 None（端点已停）
    let server_accept = tokio::time::timeout(WAIT, server.accept()).await;
    assert!(
        matches!(server_accept, Ok(None) | Err(_)),
        "accept after close must end, not hang"
    );
    accepted.await.expect("server task");
}
