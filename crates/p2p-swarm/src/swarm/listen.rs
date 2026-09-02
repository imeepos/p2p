//! 入站路径：QUIC/TCP accept 循环与门禁裁决（design §5.1 升级后分发）。

use std::sync::Arc;
use std::time::Duration;

use p2p_transport::{QuicTransport, SecureConn, TcpTransport};
use tokio::net::TcpListener;

use super::dial::insert_connection;
use super::Swarm;

/// accept 瞬时失败后的重试间隔，避免热循环。
const ACCEPT_RETRY: Duration = Duration::from_millis(10);

/// 启动两条 accept 循环；循环独占监听端，关停退出时随之丢弃（端口与连接一并释放）。
pub(super) fn spawn_accept_loops(
    swarm: &Arc<Swarm>,
    quic: QuicTransport,
    tcp: TcpTransport,
    tcp_listener: TcpListener,
) {
    tokio::spawn(quic_accept_loop(swarm.clone(), quic));
    tokio::spawn(tcp_accept_loop(swarm.clone(), tcp, tcp_listener));
}

async fn quic_accept_loop(swarm: Arc<Swarm>, quic: QuicTransport) {
    let mut shutdown = swarm.shutdown_rx.clone();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            incoming = quic.accept() => match incoming {
                Some(conn) => accept_inbound(&swarm, conn).await,
                None => {
                    // None 亦可能来自单条连接升级失败，短暂退避后继续
                    if swarm.is_stopping() {
                        break;
                    }
                    tokio::time::sleep(ACCEPT_RETRY).await;
                }
            },
        }
    }
}

async fn tcp_accept_loop(swarm: Arc<Swarm>, tcp: TcpTransport, listener: TcpListener) {
    let mut shutdown = swarm.shutdown_rx.clone();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = tcp.accept(&listener, &swarm.keypair) => match accepted {
                Ok(conn) => accept_inbound(&swarm, conn).await,
                Err(err) => {
                    tracing::warn!(error = %err, "tcp accept failed");
                    if swarm.is_stopping() {
                        break;
                    }
                    tokio::time::sleep(ACCEPT_RETRY).await;
                }
            },
        }
    }
}

/// 入站连接：门禁不放行即断链（drop 即关闭）并留告警。
async fn accept_inbound(swarm: &Arc<Swarm>, conn: SecureConn) {
    let peer = conn.remote;
    if !swarm.gate_allows(peer).await {
        tracing::warn!(%peer, "inbound connection denied by gate, dropping");
        swarm.metrics.count_gate_denial();
        return;
    }
    insert_connection(swarm, peer, conn.mux);
}
