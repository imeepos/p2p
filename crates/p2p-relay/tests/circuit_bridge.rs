//! 集成：进程内 relay（duplex mock 链路）上的电路互通、错误信号与限流断链。

use std::sync::Arc;
use std::time::Duration;

use p2p_mux::BoxedStream;
use p2p_relay::{
    errcode, mock_link_pair, CircuitId, MockLinkSource, RelayClient, RelayError, RelayLimits,
    RelayService, RelayServiceImpl,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const KB: usize = 1024;

/// 起一个进程内 relay，接好 peer-a/peer-b 两条 mock 链路，返回两客户端与链路源句柄。
/// 服务端侧链路的 peer_id 必须是对端（客户端）身份：服务端据此做配额与属主校验。
fn relay_with_two_peers(limits: RelayLimits) -> (RelayClient, RelayClient, MockLinkSource) {
    let source = MockLinkSource::new();
    let (client_a, server_a) = mock_link_pair("peer-a", "peer-a");
    let (client_b, server_b) = mock_link_pair("peer-b", "peer-b");
    source.push(Box::new(server_a));
    source.push(Box::new(server_b));
    let keep = source.clone();
    let svc = Arc::new(RelayServiceImpl::new(Box::new(source), limits));
    tokio::spawn(async move {
        let _ = svc.serve().await;
    });
    (
        RelayClient::new(Box::new(client_a)),
        RelayClient::new(Box::new(client_b)),
        keep,
    )
}

/// 双向各传一段数据：每阶段一写一读并发（mock 流缓冲小，双写并发会互堵）。
async fn exchange(
    a: &mut BoxedStream,
    b: &mut BoxedStream,
    a_to_b: &[u8],
    b_to_a: &[u8],
) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let mut got_b = vec![0u8; a_to_b.len()];
    let (w, r) = tokio::join!(a.write_all(a_to_b), b.read_exact(&mut got_b));
    w?;
    r?;
    let mut got_a = vec![0u8; b_to_a.len()];
    let (w, r) = tokio::join!(b.write_all(b_to_a), a.read_exact(&mut got_a));
    w?;
    r?;
    Ok((got_a, got_b))
}

#[tokio::test]
async fn bridged_circuit_moves_256kb_identically() {
    let (mut a, mut b, _keep) = relay_with_two_peers(RelayLimits::default());
    let cid = a
        .reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("reserve");
    // 两侧同时接入：首条被停车等配对，第二条到达即双向回 Bound
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    let (mut sa, mut sb) = (sa.expect("a connect"), sb.expect("b connect"));

    // 双向共 256KiB，逐字节一致
    let to_b: Vec<u8> = (0..128 * KB).map(|i| (i % 251) as u8).collect();
    let to_a: Vec<u8> = (0..128 * KB).map(|i| (i % 241) as u8).collect();
    let (back_a, back_b) = exchange(&mut sa, &mut sb, &to_b, &to_a)
        .await
        .expect("exchange");
    assert_eq!(back_a, to_a);
    assert_eq!(back_b, to_b);
}

#[tokio::test]
async fn unknown_circuit_rejected_with_error_signal() {
    let (mut a, mut b, _keep) = relay_with_two_peers(RelayLimits::default());
    let cid = a
        .reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("reserve");

    let outcome = b.connect(CircuitId(987_654)).await;
    let err = match outcome {
        Err(e) => e,
        Ok(_) => panic!("unknown circuit must be rejected"),
    };
    assert!(
        matches!(
            err,
            RelayError::Server {
                code: errcode::UNKNOWN_CIRCUIT,
                ..
            }
        ),
        "got {err:?}"
    );

    // 桥接确未建立：同号正常接入仍可互通
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    let (mut sa, mut sb) = (sa.expect("a connect"), sb.expect("b connect"));
    let (_, back) = exchange(&mut sa, &mut sb, b"probe", b"")
        .await
        .expect("pump");
    assert_eq!(back, b"probe");
}

#[tokio::test]
async fn per_peer_link_quota_rejects_extra_link() {
    let limits = RelayLimits {
        max_links_per_peer: 1,
        ..RelayLimits::default()
    };
    let source = MockLinkSource::new();
    let (c1, s1) = mock_link_pair("peer-a", "peer-a");
    let (c2, s2) = mock_link_pair("peer-a", "peer-a");
    let keep = source.clone();
    let svc = Arc::new(RelayServiceImpl::new(Box::new(source), limits));
    tokio::spawn(async move {
        let _ = svc.serve().await;
    });

    let mut first = RelayClient::new(Box::new(c1));
    let mut second = RelayClient::new(Box::new(c2));
    // 先挂第一条链路并确认可用，避免两条链路的注册竞态
    keep.push(Box::new(s1));
    first
        .reserve(Duration::from_secs(60), "")
        .await
        .expect("first link admitted");
    keep.push(Box::new(s2));
    let outcome = second.reserve(Duration::from_secs(60), "").await;
    let err = match outcome {
        Err(e) => e,
        Ok(_) => panic!("second link must be rejected"),
    };
    assert!(
        matches!(
            err,
            RelayError::Server {
                code: errcode::PEER_LIMIT,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[tokio::test]
async fn egress_quota_cuts_circuit_and_signals() {
    let limits = RelayLimits {
        egress_burst: 1024,
        egress_bytes_per_sec: 1024,
        ..RelayLimits::default()
    };
    let (mut a, mut b, _keep) = relay_with_two_peers(limits);
    let cid = a
        .reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("reserve");
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    let (mut sa, mut sb) = (sa.expect("a connect"), sb.expect("b connect"));

    // 桶容量内的小流量正常通过
    let (_, back) = exchange(&mut sa, &mut sb, &[0u8; 512], b"")
        .await
        .expect("within quota");
    assert_eq!(back, &[0u8; 512]);

    // 超过桶容量：出口写失败即断链，读端收到显式错误
    let big = vec![7u8; 64 * KB];
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        let mut buf = vec![0u8; 64 * KB];
        let (wr, rd) = tokio::join!(sa.write_all(&big), sb.read_exact(&mut buf));
        (wr, rd)
    })
    .await
    .expect("no hang");
    assert!(
        outcome.0.is_err() || outcome.1.is_err(),
        "circuit must be cut after egress excess"
    );
}

#[tokio::test]
async fn foreign_joiner_cannot_attach_anothers_circuit() {
    // 审查 M2 回归：即便拿到真实 cid，未列入 allowed_joiner 的第三方也被拒
    let (mut a, mut b, keep) = relay_with_two_peers(RelayLimits::default());
    let cid = a
        .reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("reserve");

    let (client_c, server_c) = mock_link_pair("peer-c", "peer-c");
    keep.push(Box::new(server_c));
    let mut c = RelayClient::new(Box::new(client_c));
    let outcome = c.connect(cid).await;
    let err = match outcome {
        Err(e) => e,
        Ok(_) => panic!("foreign joiner must be rejected"),
    };
    assert!(
        matches!(
            err,
            RelayError::Server {
                code: errcode::FORBIDDEN_JOINER,
                ..
            }
        ),
        "got {err:?}"
    );

    // 属主与被允许者仍可正常建桥
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    let (mut sa, mut sb) = (sa.expect("a connect"), sb.expect("b connect"));
    let (_, back) = exchange(&mut sa, &mut sb, b"probe", b"")
        .await
        .expect("pump");
    assert_eq!(back, b"probe");
}
