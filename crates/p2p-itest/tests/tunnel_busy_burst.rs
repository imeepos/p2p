//! W-T5 busy 栅栏对账红绿（规范页 §5.2 + gui-contract §19.8，wt3c 实证驱动）：
//! - 裁决探针：默认配置吸收 wt3c 实测浏览器 burst 峰值 14 并发、零 busy
//!   （默认许可 4 时代确定性红：超 4 部分必 busy，EVIDENCE-wt3c.md）；
//! - 语义探针：显式低许可下超限部分 busy，且每条 busy 逐条落终态审计
//!   （契约 §3 禁止静默断言：拒绝也是一条终态记录）。

mod tunnel_common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use p2p_tunnel::{
    TunnelAuditOutcome, TunnelClient, TunnelError, TunnelErrorCode, TunnelServeConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Notify};
use tunnel_common::{rig, Rig, STEP, TARGET};

/// wt3c 实测浏览器 burst 峰值（EVIDENCE-wt3c.md：冷加载 11 并行 + API burst 14）。
const BURST: usize = 14;
const PAYLOAD: usize = 8 * 1024;

fn uid(i: usize) -> String {
    format!("{i:016x}")
}

fn ticket_for(i: usize) -> p2p_tunnel::TunnelTicket {
    p2p_tunnel::TunnelTicket::new(uid(i), TARGET, "ab".repeat(16)).unwrap()
}

/// 预置 n 条目标连接并各挂回环 echo（responder 拨进来的字节原样回写）。
async fn feed_echo(setup: &Rig, n: usize) {
    for _ in 0..n {
        let target_end = setup.dialer.feed().await;
        tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(target_end);
            let _ = tokio::io::copy(&mut r, &mut w).await;
            let _ = w.shutdown().await;
        });
    }
}

/// 单会话任务：open → 写 payload → 读回逐字节比对 → 顶住 close 门（held 时
/// permit 不还）→ 结果经 channel 上报。Rejected 也走 channel（busy 是合法结果）。
async fn one_session(
    client: TunnelClient<tunnel_common::NodeFactory>,
    peer: p2p_identity::PeerId,
    i: usize,
    hold: Arc<Notify>,
    tx: mpsc::Sender<Result<usize, TunnelError>>,
) {
    let payload: Vec<u8> = (0..PAYLOAD).map(|b| (b % 251) as u8).collect();
    let expected = payload.clone();
    let mut io = match client.open(peer, &ticket_for(i)).await {
        Ok(io) => io,
        Err(e) => {
            let _ = tx.send(Err(e)).await;
            return;
        }
    };
    if let Err(e) = io.write_all(&payload).await {
        let _ = tx
            .send(Err(TunnelError::Io(std::io::Error::other(e.to_string()))))
            .await;
        return;
    }
    let mut got = vec![0u8; expected.len()];
    if let Err(e) = io.read_exact(&mut got).await {
        let _ = tx
            .send(Err(TunnelError::Io(std::io::Error::other(e.to_string()))))
            .await;
        return;
    }
    assert_eq!(got, expected, "会话 {i} 回程字节一致");
    let _ = tx.send(Ok(i)).await;
    hold.notified().await; // 顶住会话不收口（permit 占用中），由驱动方放行
    drop(io);
}

/// 汇总一轮 burst 的结果分类。
async fn collect(
    mut rx: mpsc::Receiver<Result<usize, TunnelError>>,
    total: usize,
) -> (usize, usize, Vec<String>) {
    let (mut ok, mut busy, mut other) = (0, 0, Vec::new());
    for _ in 0..total {
        match rx.recv().await {
            Some(Ok(_)) => ok += 1,
            Some(Err(TunnelError::Rejected {
                code: TunnelErrorCode::Busy,
                ..
            })) => busy += 1,
            Some(Err(e)) => other.push(e.to_string()),
            None => break,
        }
    }
    (ok, busy, other)
}

/// 轮询审计账直到落满 n 条记录（拒绝同步落账，served 在泵收口后落账）。
async fn wait_audit_count(setup: &Rig, n: usize) -> Vec<p2p_tunnel::TunnelAuditRecord> {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        let records = setup.audit.snapshot();
        if records.len() >= n {
            return records;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "审计未落满 {n} 条：{:?}",
            setup.audit.snapshot()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 裁决探针（默认配置）：wt3c 峰值 14 并发全过、零 busy。默认许可 4 时代
/// 本用例确定性红（10 条 busy），2026-09-12 上调裁决后转绿。
#[tokio::test]
async fn default_gate_absorbs_wt3c_burst_peak_without_busy() {
    let cfg = TunnelServeConfig {
        allowlist: HashSet::from([TARGET.to_string()]),
        ..Default::default()
    };
    let setup = rig("burst-default", cfg, true).await;
    feed_echo(&setup, BURST).await;
    let (tx, rx) = mpsc::channel(BURST);
    let hold = Arc::new(Notify::new());
    let mut tasks = Vec::new();
    for i in 0..BURST {
        let (client, hold, tx) = (setup.client.clone(), hold.clone(), tx.clone());
        tasks.push(tokio::spawn(one_session(client, setup.a_peer, i, hold, tx)));
    }
    let (ok, busy, other) = collect(rx, BURST).await;
    assert!(other.is_empty(), "非 busy 意外失败: {other:?}");
    assert_eq!(
        busy, 0,
        "默认配置必须吸收 wt3c 峰值 burst（busy={busy} ok={ok}）"
    );
    assert_eq!(ok, BURST);
    hold.notify_waiters();
    for task in tasks {
        task.await.unwrap();
    }
    let records = wait_audit_count(&setup, BURST).await;
    assert!(
        records
            .iter()
            .all(|r| r.outcome == TunnelAuditOutcome::Served),
        "全部会话落 served 终态: {records:?}"
    );
}

/// 语义探针（显式低许可）：许可 2 + burst 4 → 恰 2 served 2 busy，且每条
/// busy 逐条落终态审计（Rejected(Busy)），禁止静默。
#[tokio::test]
async fn over_cap_burst_rejects_busy_with_terminal_audit_each() {
    let cfg = TunnelServeConfig {
        allowlist: HashSet::from([TARGET.to_string()]),
        max_concurrent: 2,
        ..Default::default()
    };
    let setup = rig("burst-cap2", cfg, true).await;
    feed_echo(&setup, 2).await;
    let burst = 4;
    let (tx, rx) = mpsc::channel(burst);
    let hold = Arc::new(Notify::new());
    let mut tasks = Vec::new();
    for i in 0..burst {
        let (client, hold, tx) = (setup.client.clone(), hold.clone(), tx.clone());
        tasks.push(tokio::spawn(one_session(client, setup.a_peer, i, hold, tx)));
    }
    let (ok, busy, other) = collect(rx, burst).await;
    assert!(other.is_empty(), "非 busy 意外失败: {other:?}");
    assert_eq!(ok, 2, "许可内会话全过");
    assert_eq!(busy, 2, "超许可部分恰为 busy");
    hold.notify_waiters();
    for task in tasks {
        task.await.unwrap();
    }
    let records = wait_audit_count(&setup, burst).await;
    let served = records
        .iter()
        .filter(|r| r.outcome == TunnelAuditOutcome::Served)
        .count();
    let busy_rejected = records
        .iter()
        .filter(|r| r.outcome == TunnelAuditOutcome::Rejected(TunnelErrorCode::Busy))
        .count();
    assert_eq!(
        (served, busy_rejected),
        (2, 2),
        "逐条落终态: {records:?}"
    );
}
