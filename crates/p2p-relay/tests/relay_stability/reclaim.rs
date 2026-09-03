//! 回收语义回归：空闲拆桥（消融点 1）/ 活跃反例 / 控制流关闭回收（E4）。

use std::time::Duration;

use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{
    errcode, relay_msg::Kind, write_msg, RelayClient, RelayKeepalive, RelayLimits, RelayLink,
    RelayMsg,
};
use tokio::io::AsyncReadExt;

use crate::support::{
    bridged_pair, ka, manual_reserve, pump, read_frame, relay_pair_with, spawn_relay,
};

/// 空闲回收（含消融点 1）：双向静默超过 idle TTL 即拆桥，配额随槽位退役回吐。
#[tokio::test]
async fn idle_bridged_circuit_reclaimed_after_ttl() {
    // per-peer 配额 2：桥接期被双方打满（reserve+connect），回收后必须能再次发放
    let limits = RelayLimits {
        max_circuits_per_peer: 2,
        ..RelayLimits::default()
    };
    let (mut a, mut b, _keep) = relay_pair_with(ka(250, 10_000, 5_000, 3, 45_000), limits);
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    // 持续静默直到收敛；消融点 1 删回收分支则此 read 挂死 -> 5s 超时 panic 变红。
    let drain = async {
        let (mut ba, mut bb) = ([0u8; 4], [0u8; 4]);
        tokio::join!(sa.read(&mut ba), sb.read(&mut bb))
    };
    let converged = tokio::time::timeout(Duration::from_secs(5), drain)
        .await
        .expect("must reclaim in 5s");
    for r in [converged.0, converged.1] {
        assert!(r.is_err() || r.unwrap_or(0) == 0, "idle bridge must close");
    }
    // 配额回吐断言：消融后配额仍被占用，此处变红。
    a.reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("quota back");
    b.reserve(Duration::from_secs(60), "")
        .await
        .expect("quota back");
}

/// 反例：有数据流动的电路绝不被误收（静默间隙恒小于 idle TTL）。
#[tokio::test]
async fn active_bridged_circuit_never_reclaimed() {
    // E9 时序去脆弱化：窗口缩至 idle=100ms/gap=20ms（1:5），span 覆盖 ~10×TTL。
    let (mut a, mut b, _keep) =
        relay_pair_with(ka(100, 10_000, 5_000, 3, 45_000), RelayLimits::default());
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    for i in 0..40u8 {
        pump(&mut sa, &mut sb, &[i; 16]).await;
        pump(&mut sb, &mut sa, &[i; 16]).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    pump(&mut sa, &mut sb, b"still-alive").await;
}

/// 既有回收语义回归（E4）：控制流关闭回收未桥接电路，桥接中的不受牵连。
#[tokio::test]
async fn control_close_keeps_bridged_rejects_parked() {
    let source = MockLinkSource::new();
    let svc = spawn_relay(&source, RelayLimits::default(), RelayKeepalive::default());
    let (a_cli, a_srv) = mock_link_pair("peer-a", "peer-a");
    let (b_cli, b_srv) = mock_link_pair("peer-b", "peer-b");
    source.push(Box::new(a_srv));
    source.push(Box::new(b_srv));
    let mut b = RelayClient::new(Box::new(b_cli));
    // 裸控制流注册 + 裸流停车 + b 客户端接入配对成桥
    let mut ctrl = a_cli.open_stream().await.expect("open control");
    let cid = manual_reserve(&mut ctrl, 3600, "peer-b").await;
    let mut sa = a_cli.open_stream().await.expect("open circuit stream");
    write_msg(&mut sa, &RelayMsg::connect(cid.0))
        .await
        .expect("write connect");
    let mut sb = b.connect(cid).await.expect("b joins circuit");
    assert!(
        matches!(read_frame(&mut sa).await.kind, Some(Kind::Bound(_))),
        "expected Bound"
    );
    pump(&mut sa, &mut sb, b"before-close").await;
    // 控制流关闭（EOF）：桥接电路不受牵连（E4），数据继续互通
    drop(ctrl);
    pump(&mut sa, &mut sb, b"after-close").await;
    // 未桥接电路随控制流关闭回收（E4）。停车/EOF 竞态落在哪侧都给显式信号：
    // 持车方收 CIRCUIT_EXPIRED，在途 connect 收 UNKNOWN_CIRCUIT，不静默悬挂。
    let mut ctrl2 = a_cli.open_stream().await.expect("reopen control");
    let cid2 = manual_reserve(&mut ctrl2, 3600, "").await;
    assert_eq!(svc.metrics().circuits_active, 2, "cid2 slot registered");
    let mut parked = a_cli.open_stream().await.expect("open parked stream");
    write_msg(&mut parked, &RelayMsg::connect(cid2.0))
        .await
        .expect("write connect 2");
    drop(ctrl2);
    // 轮询槽位水位 2 -> 1：确定性等待回收完成（yield 自旋，零真实时钟依赖）
    for _ in 0..50_000 {
        if svc.metrics().circuits_active == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(svc.metrics().circuits_active, 1, "watermark never reached");
    match read_frame(&mut parked).await.kind {
        Some(Kind::Reject(r)) => assert!(
            r.code == errcode::CIRCUIT_EXPIRED || r.code == errcode::UNKNOWN_CIRCUIT,
            "expected explicit reclaim signal, got {r:?}"
        ),
        other => panic!("expected explicit reject, got {other:?}"),
    }
}
