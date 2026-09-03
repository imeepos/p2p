//! 查询退避调度回归：快启动档/稳态档边界与窗口内查询计数。

use super::*;
use crate::rendezvous::link::mock::{conn_from_duplex, MockLink};

#[test]
fn default_query_schedule_backs_off_after_boot_rounds() {
    // 全量轮询成本 O(N)：稳态档必须 30s ± 20%，启动期保留快速档尽快填表
    let (client_side, _server_side) = tokio::io::duplex(64);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let cfg = RendezvousConfig::new("room-a", Keypair::generate(), link);
    assert_eq!(cfg.query_boot_interval, Duration::from_secs(5));
    assert_eq!(cfg.query_boot_rounds, 2);
    assert_eq!(cfg.query_interval, Duration::from_secs(30));
    assert_eq!(cfg.next_query_delay(0), Duration::from_secs(5));
    assert_eq!(cfg.next_query_delay(1), Duration::from_secs(5));
    for round in 2..12 {
        let d = cfg.next_query_delay(round);
        assert!(
            d >= Duration::from_secs(24) && d <= Duration::from_secs(36),
            "稳态档应落在 30s±20%，实际 {d:?}"
        );
    }
}

#[tokio::test]
async fn query_count_in_window_reflects_backoff() {
    // 行为回归：boot 2 轮 10ms 后进入 400ms 稳态档——650ms 窗口内
    // 查询数应为 初始1 + boot2 + 稳态1~2 = 4~5；恒定 10ms 轮询会是 ~65 次
    use crate::rendezvous::messages::request;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let mut config = RendezvousConfig::new("room-a", Keypair::generate(), link);
    config.query_boot_interval = Duration::from_millis(10);
    config.query_boot_rounds = 2;
    config.query_interval = Duration::from_millis(400);
    config.register_interval = Duration::from_secs(3600); // 只统计查询
    let client = RendezvousClient::new(config);

    let count = Arc::new(AtomicUsize::new(0));
    let count_cl = count.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        loop {
            let req = match server.recv_msg::<Request>().await {
                Ok(r) => r,
                Err(_) => break,
            };
            if matches!(req.kind, Some(request::Kind::Query(_))) {
                count_cl.fetch_add(1, Ordering::SeqCst);
            }
            let _ = server.send_msg(&Response::ok()).await;
        }
    });

    let (tx, _rx) = mpsc::channel(16);
    let cache = MemCache::new();
    let _ = tokio::time::timeout(
        Duration::from_millis(650),
        client.connect_and_loop(&tx, &cache),
    )
    .await;
    let n = count.load(Ordering::SeqCst);
    assert!(
        (3..=6).contains(&n),
        "650ms 内查询数应反映退避（3~6），实际 {n}"
    );
    server_task.abort();
}
