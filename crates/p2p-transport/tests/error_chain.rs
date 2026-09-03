//! E7-K2 回归：传输层错误映射必须保留内层错误链。
//!
//! 断言调用方可沿 TransportError::source() 逐层还原内层错误类型与文案。
//! 消融证明：撤掉映射修复（装箱改回 to_string 拍平）后本文件用例转红。

use std::net::IpAddr;
use std::time::Duration;

use p2p_identity::Keypair;
use p2p_transport::{TcpTransport, Transport, TransportAddr, TransportError};

const DIAL_WAIT: Duration = Duration::from_secs(5);

fn tcp(port: u16) -> TransportAddr {
    TransportAddr::Tcp {
        ip: IpAddr::from([127, 0, 0, 1]),
        port,
    }
}

/// 沿 source 链逐层下探，检查目标类型是否可还原。
fn chain_contains<C: std::error::Error + 'static>(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur = Some(err);
    while let Some(e) = cur {
        if e.downcast_ref::<C>().is_some() {
            return true;
        }
        cur = e.source();
    }
    false
}

#[tokio::test]
async fn tcp_dial_refused_keeps_io_error_source() {
    // loopback 上无监听端口的连接拒绝是确定性的（无防火墙干扰）
    let client = TcpTransport::new();
    let kp = Keypair::generate();
    let err = match client.dial(&tcp(1), &kp, None).await {
        Err(e) => e,
        Ok(_) => panic!("dial to closed port must fail"),
    };
    match &err {
        TransportError::DialChained { addr, .. } => {
            assert!(addr.contains("127.0.0.1/t1"), "addr preserved, got {addr}");
        }
        other => panic!("expected DialChained, got {other:?}"),
    }
    assert!(
        chain_contains::<std::io::Error>(&err),
        "source 链必须能还原内层 io::Error，实际: {err}"
    );
}

#[tokio::test]
async fn quic_dial_invalid_remote_keeps_connect_error_source() {
    let client = p2p_transport::QuicTransport::new().expect("client transport");
    let kp = Keypair::generate();
    // 拨号端点绑 IPv4，拨 IPv6 地址必被 quinn 以 ConnectError 拒绝（确定性）
    let addr = TransportAddr::Quic {
        ip: IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        port: 1,
    };
    let err = match tokio::time::timeout(DIAL_WAIT, client.dial(&addr, &kp, None)).await {
        Ok(Err(e)) => e,
        Ok(Ok(_)) => panic!("invalid remote must fail"),
        Err(_) => panic!("dial must fail fast, not hang"),
    };
    match &err {
        TransportError::DialChained { .. } => {}
        other => panic!("expected DialChained, got {other:?}"),
    }
    assert!(
        chain_contains::<quinn::ConnectError>(&err),
        "source 链必须能还原 quinn::ConnectError，实际: {err}"
    );
}

#[tokio::test]
async fn tcp_wrong_family_dial_keeps_text_variant_contract() {
    // 冻结契约回归：契约性拒绝（无内层错误对象）保留既有 Dial 文本变体
    let client = TcpTransport::new();
    let kp = Keypair::generate();
    let addr = TransportAddr::Quic {
        ip: IpAddr::from([127, 0, 0, 1]),
        port: 1,
    };
    match client.dial(&addr, &kp, None).await {
        Err(TransportError::Dial { reason, .. }) => {
            assert!(
                reason.contains("cannot dial a quic address"),
                "reason preserved, got {reason}"
            );
        }
        Err(other) => panic!("expected text Dial variant, got: {other}"),
        Ok(_) => panic!("wrong family must be rejected"),
    }
}
