//! 客户端健康观测：Reserved 捎带 load 即时可见，keepalive 往返产出 RTT EMA。

use std::time::Duration;

use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{RelayClient, RelayKeepalive, RelayLimits, RelayServiceImpl};

#[tokio::test]
async fn reserved_load_visible_and_keepalive_measures_rtt() {
    let source = MockLinkSource::new();
    let _svc = RelayServiceImpl::spawn(
        Box::new(source.clone()),
        RelayLimits {
            max_total_circuits: 2,
            ..RelayLimits::default()
        },
    );
    let (ca, sa) = mock_link_pair("peer-a", "relay");
    source.push(Box::new(sa));
    let ka = RelayKeepalive {
        interval: Duration::from_millis(40),
        timeout: Duration::from_secs(1),
        ..RelayKeepalive::default()
    };
    let mut client = RelayClient::with_keepalive(Box::new(ca), ka);
    assert!(client.health().is_none(), "控制链路建立前无健康快照");

    client
        .reserve(Duration::from_secs(60), "")
        .await
        .expect("reserve");
    let h = client.health().expect("health after reserve");
    assert_eq!(h.load_permille, 500, "1/2 电路占用应经 Reserved 即时可见");

    // 等多个 keepalive 周期：RTT EMA 应已产出且水位持续刷新
    tokio::time::sleep(Duration::from_millis(200)).await;
    let h = client.health().expect("health alive");
    assert!(
        h.rtt_ema_ms >= 1,
        "keepalive 往返必须产出 RTT EMA（亚毫秒计 1）"
    );
    assert_eq!(h.load_permille, 500);

    let handle = client.health_handle().expect("shared handle");
    assert_eq!(handle.snapshot(), client.health().unwrap());
}
