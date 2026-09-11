//! W-T2 双节点 itest：/p2p-base/tunnel/1 被访侧真实链路（真两 Node、真拨号）。
//! 绿：open → ticket → ack → 双向字节往返（>1 MiB、分块合并）、HTTP 形态字节
//! 原样透传、半关方向独立。红：白名单 / 按次开关 / 坏票据 / 并发超限全部显式
//! 错误帧（契约 §3：禁止静默断流）。
//!
//! ≤64 KiB 出站分块的帧界断言在 crates/p2p-tunnel/src/pump.rs 单测：mux 流上
//! 无分帧观测点，itest 侧以「三段异形写 → 目标侧字节流原样」断言合并语义。

mod tunnel_common;

use std::collections::HashSet;

use p2p_tunnel::{TunnelAuditOutcome, TunnelError, TunnelErrorCode, TunnelServeConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tunnel_common::{
    expect_reject, reject_code, rig, started_rig, ticket, wait_audit, OTHER, STEP, TARGET, UID,
};

/// 绿：ack 后双向字节往返；>1 MiB 单向大流量三段异形写原样到达并回程一致；
/// audit 记 outcome=Served 且 bytes 计数精确。
#[tokio::test]
async fn g1_large_bidirectional_roundtrip_and_audit() {
    let rig = started_rig("g1").await;
    let target_end = rig.dialer.feed().await;
    let echo = tokio::spawn(async move {
        let (mut r, mut w) = tokio::io::split(target_end);
        let _ = tokio::io::copy(&mut r, &mut w).await;
        let _ = w.shutdown().await;
    });
    let io = rig
        .client
        .open(rig.a_peer, &ticket(TARGET))
        .await
        .expect("open tunnel");
    let payload: Vec<u8> = (0..=1_048_600u32).map(|i| (i % 251) as u8).collect();
    let expected = payload.clone();
    let push = tokio::spawn(async move {
        // 读写并发（真实反代语义）：管道缓冲 < 1 MiB，先写后读会自锁
        let (mut io_r, mut io_w) = tokio::io::split(io);
        let writer = async {
            io_w.write_all(&payload[..500_000]).await?;
            io_w.write_all(&payload[500_000..]).await?;
            io_w.shutdown().await
        };
        let reader = async {
            let mut got = Vec::new();
            io_r.read_to_end(&mut got).await?;
            Ok::<_, std::io::Error>(got)
        };
        let (_, got) = tokio::join!(writer, reader);
        got
    });
    let got = push.await.unwrap().expect("roundtrip");
    assert_eq!(got.len(), expected.len(), "回程字节量一致");
    assert_eq!(got, expected, "回程与去程逐字节一致（分块合并语义）");
    echo.await.unwrap();
    let rec = wait_audit(&rig, UID).await;
    assert_eq!(rec.outcome, TunnelAuditOutcome::Served, "{rec:?}");
    let total = expected.len() as u64;
    assert_eq!(
        (rec.bytes_in, rec.bytes_out),
        (total, total),
        "双向字节计数: {rec:?}"
    );
    assert_eq!(rec.target, TARGET);
    assert_eq!(rec.peer_id, rig.b.local_peer_id().to_string(), "身份取握手 PeerId");
}

/// 绿：HTTP/1.1 形态字节流原样过隧道（固定字节夹具，哑泵不解释 HTTP 语义）。
#[tokio::test]
async fn g2_http_shape_bytes_pass_through_verbatim() {
    let rig = started_rig("g2").await;
    let mut target_end = rig.dialer.feed().await;
    let mut io = rig
        .client
        .open(rig.a_peer, &ticket(TARGET))
        .await
        .expect("open tunnel");
    let request = b"POST /api?v=1 HTTP/1.1\r\nHost: 127.0.0.1:8014\r\nContent-Length: 5\r\n\r\nhello";
    let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
    io.write_all(request).await.unwrap();
    io.shutdown().await.unwrap();
    let mut buf = vec![0u8; request.len()];
    target_end.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, request, "请求字节原样到达目标");
    target_end.write_all(response).await.unwrap();
    target_end.shutdown().await.unwrap();
    let mut got = Vec::new();
    io.read_to_end(&mut got).await.unwrap();
    assert_eq!(&got, response, "响应字节原样返回");
    let rec = wait_audit(&rig, UID).await;
    assert_eq!(rec.outcome, TunnelAuditOutcome::Served, "{rec:?}");
}

/// 红：非白名单目标 → 显式 target_not_allowed 错误帧（非静默断流），审计留痕。
#[tokio::test]
async fn r1_non_allowlist_target_rejected_with_error_frame() {
    let rig = started_rig("r1").await;
    let err = expect_reject(rig.client.open(rig.a_peer, &ticket(OTHER))).await;
    eprintln!("[evidence r1] open -> {err}");
    let code = reject_code(err);
    assert_eq!(
        code,
        TunnelErrorCode::TargetNotAllowed,
        "实际错误码: {code}"
    );
    let rec = wait_audit(&rig, UID).await;
    assert_eq!(
        rec.outcome,
        TunnelAuditOutcome::Rejected(TunnelErrorCode::TargetNotAllowed),
        "{rec:?}"
    );
    assert_eq!(rec.target, OTHER);
    assert_eq!((rec.bytes_in, rec.bytes_out), (0, 0), "拒绝路径零字节");
}

/// 红：按次开关关闭 → 被拒（shutdown 码）且审计有记录。
#[tokio::test]
async fn r2_gate_off_rejected_and_audited() {
    let cfg = TunnelServeConfig {
        allowlist: HashSet::from([TARGET.to_string()]),
        ..Default::default()
    };
    let rig = rig("r2", cfg, false).await;
    let err = expect_reject(rig.client.open(rig.a_peer, &ticket(TARGET))).await;
    eprintln!("[evidence r2] open -> {err}");
    let code = reject_code(err);
    assert_eq!(code, TunnelErrorCode::Shutdown, "实际错误码: {code}");
    let rec = wait_audit(&rig, UID).await;
    assert_eq!(
        rec.outcome,
        TunnelAuditOutcome::Rejected(TunnelErrorCode::Shutdown),
        "{rec:?}"
    );
}

/// 红：过期 ts / 未来 ts / 版本不符 → bad_ticket。
#[tokio::test]
async fn r3_bad_ticket_version_and_stale_ts() {
    let rig = started_rig("r3").await;
    let mut stale_version = ticket(TARGET);
    stale_version.v = 2;
    let err = expect_reject(rig.client.open(rig.a_peer, &stale_version)).await;
    eprintln!("[evidence r3a] open -> {err}");
    assert_eq!(reject_code(err), TunnelErrorCode::BadTicket);
    let now = p2p_tunnel::now_unix_secs();
    for stale in [ticket(TARGET).with_ts(now - 301), ticket(TARGET).with_ts(now + 301)] {
        let err = expect_reject(rig.client.open(rig.a_peer, &stale)).await;
        eprintln!("[evidence r3b] open -> {err}");
        assert_eq!(reject_code(err), TunnelErrorCode::BadTicket);
    }
    let recs = rig
        .audit
        .snapshot()
        .into_iter()
        .filter(|r| r.session_id == UID)
        .collect::<Vec<_>>();
    assert_eq!(recs.len(), 3, "三次拒绝均落审计: {recs:?}");
}

/// 红：并发超限 → busy；会话整条关闭后许可归还（可再开、再超限）。
#[tokio::test]
async fn r4_concurrency_cap_busy_then_release() {
    let cfg = TunnelServeConfig {
        allowlist: HashSet::from([TARGET.to_string()]),
        max_concurrent: 1,
        ..Default::default()
    };
    let rig = rig("r4", cfg, true).await;
    let t1 = rig.dialer.feed().await;
    let io1 = rig
        .client
        .open(rig.a_peer, &ticket(TARGET))
        .await
        .expect("首个会话应开");
    let err = expect_reject(rig.client.open(rig.a_peer, &ticket(TARGET))).await;
    eprintln!("[evidence r4a] open -> {err}");
    assert_eq!(reject_code(err), TunnelErrorCode::Busy, "实际错误码应 busy");
    drop(io1);
    drop(t1);
    let t2 = rig.dialer.feed().await;
    let mut reopened = None;
    for _ in 0..100 {
        match rig.client.open(rig.a_peer, &ticket(TARGET)).await {
            Ok(io) => {
                reopened = Some(io);
                break;
            }
            Err(TunnelError::Rejected {
                code: TunnelErrorCode::Busy,
                ..
            }) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
            Err(e) => panic!("许可归还期只允许 busy: {e:?}"),
        }
    }
    let io3 = reopened.expect("许可归还后应可再开");
    let err = expect_reject(rig.client.open(rig.a_peer, &ticket(TARGET))).await;
    eprintln!("[evidence r4b] open -> {err}");
    assert_eq!(reject_code(err), TunnelErrorCode::Busy);
    drop(io3);
    drop(t2);
}

/// 半关：访侧 finish 后回程仍流动；目标侧 finish 后访侧见 EOF；双向 finish
/// 后整隧道干净关闭（audit Served）。
#[tokio::test]
async fn h1_half_close_directions_independent() {
    let rig = started_rig("h1").await;
    let mut target_end = rig.dialer.feed().await;
    let mut io = rig
        .client
        .open(rig.a_peer, &ticket(TARGET))
        .await
        .expect("open tunnel");
    io.write_all(b"REQ-BODY").await.unwrap();
    io.shutdown().await.unwrap();
    let mut req = Vec::new();
    target_end.read_to_end(&mut req).await.unwrap();
    assert_eq!(req, b"REQ-BODY", "访侧 finish = 目标侧读 EOF");
    target_end.write_all(b"RESP-AFTER-FINISH").await.unwrap();
    let mut buf = [0u8; 17];
    io.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"RESP-AFTER-FINISH", "finish 后回程仍流动");
    target_end.shutdown().await.unwrap();
    let mut rest = Vec::new();
    io.read_to_end(&mut rest).await.unwrap();
    assert!(rest.is_empty(), "回程 EOF 呈现");
    let rec = wait_audit(&rig, UID).await;
    assert_eq!(rec.outcome, TunnelAuditOutcome::Served, "{rec:?}");
    assert_eq!((rec.bytes_in, rec.bytes_out), (8, 17), "{rec:?}");
    let _ = STEP;
}
