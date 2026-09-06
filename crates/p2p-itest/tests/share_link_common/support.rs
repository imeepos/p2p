//! SHARE 波夹具·admin 客户端与断言工具：真实 TCP + Bearer 打 owner admin HTTP，
//! Ready/Denied 解包、审计条件等待、ndjson 行编解码与监狱路径换算。
//! 只放装置与有界等待（显式 timeout），断言留在 share_link_wave.rs。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use acp_agent::{AuditEvent, CaptureAudit};
use acp_common::{Scope, ServerHello, ShareDenyKind};
use p2p::BoxedStream;
use serde_json::Value;

use super::{OwnerRig, STEP};

/// admin HTTP 客户端：真实 TCP + Bearer；Connection: close 读到 EOF。
pub async fn admin_call(
    addr: SocketAddr,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, Value) {
    let mut tcp = tokio::net::TcpStream::connect(addr)
        .await
        .expect("admin connect");
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    tcp.write_all(req.as_bytes()).await.expect("admin write");
    let mut raw = Vec::new();
    tokio::time::timeout(STEP, tcp.read_to_end(&mut raw))
        .await
        .expect("admin read timeout")
        .expect("admin read");
    let text = String::from_utf8(raw).expect("utf8 admin reply");
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or_default();
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

/// admin POST /shares：非 200 即 panic，返回创建响应 JSON。
pub async fn create_share(rig: &OwnerRig, body: Value) -> Value {
    let (status, created) = admin_call(
        rig.admin_addr,
        &rig.admin_token,
        "POST",
        "/shares",
        Some(&body.to_string()),
    )
    .await;
    assert_eq!(status, 200, "share create failed: {created}");
    created
}

/// Ready 解包（断言用）：denied 即 panic，denied 码随消息可见。
pub fn expect_ready(verdict: super::DialVerdict, what: &str) -> (Scope, BoxedStream) {
    match verdict {
        super::DialVerdict::Ready {
            hello: ServerHello::Ready { ready },
            stream,
        } => (ready.scope, stream),
        super::DialVerdict::Ready { .. } => panic!("{what}: unexpected hello shape"),
        super::DialVerdict::Denied { code } => panic!("{what}: dial denied: {code}"),
    }
}

/// Denied 解包（断言用）：拿到 denied 帧错误码。
pub fn expect_denied(verdict: super::DialVerdict, what: &str) -> String {
    match verdict {
        super::DialVerdict::Denied { code } => code,
        super::DialVerdict::Ready { .. } => panic!("{what}: expected denial, got ready"),
    }
}

/// scope=sandbox 监狱期望路径（canonicalize 对齐 agent 侧 ensure_dir）。
pub fn sandbox_jail(rig: &OwnerRig, peer: &str) -> PathBuf {
    let jail = rig.deps.config.sandbox_root().join(peer);
    std::fs::canonicalize(&jail).expect("jail dir must exist after spawn")
}

/// 条件等待审计事件：替代盲睡，超时带全量快照便于排障。
pub async fn wait_audit(
    audit: &CaptureAudit,
    pred: impl Fn(&AuditEvent) -> bool + Copy,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if audit.contains(pred) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "audit wait timed out: {:?}",
            audit.snapshot()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 条件等待 share-redeemed（兑换激活）事件。
pub async fn wait_share_redeemed(rig: &OwnerRig, share_id: &str) {
    wait_audit(
        rig.audit.as_ref(),
        |ev| matches!(ev, AuditEvent::ShareRedeemed { share_id: id, .. } if id == share_id),
        15,
    )
    .await;
}

/// 条件等待「兑换被拒」事件（按 ShareDenyKind 匹配，覆盖四条拒绝审计键）。
pub async fn wait_redeem_denied(rig: &OwnerRig, deny: ShareDenyKind) {
    wait_audit(
        rig.audit.as_ref(),
        |ev| matches!(ev, AuditEvent::ShareRedeemDenied { kind, .. } if *kind == deny),
        15,
    )
    .await;
}

/// libtest 捕获 print!/eprint!，直写 stderr 才能无条件留下 SKIP 信号。
pub fn skip_signal(reason: &str) {
    use std::io::Write as _;
    let _ =
        std::io::stderr().write_all(format!("SKIP: real dsh unavailable: {reason}\n").as_bytes());
}

/// 带时限读一行：EOF 与超时统一返回 None（真链路可用性判定依赖此语义）。
pub async fn line_within(stream: &mut BoxedStream, secs: u64) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(secs), read_line(stream))
        .await
        .ok()
        .flatten()
}

pub async fn send_line<W: tokio::io::AsyncWrite + Unpin + Send>(writer: &mut W, line: &str) {
    use tokio::io::AsyncWriteExt as _;
    for frame in acp_common::frames(line.as_bytes()) {
        p2p_protocol::write_frame(writer, frame)
            .await
            .expect("write frame");
    }
    writer.flush().await.expect("flush");
}

/// 读一条 ndjson 行；流关闭返回 None。
pub async fn read_line<R: tokio::io::AsyncRead + Unpin + Send>(reader: &mut R) -> Option<String> {
    let mut reassembler = acp_common::LineReassembler::new();
    loop {
        if let Some(line) = reassembler.take_line() {
            return Some(String::from_utf8(line).expect("utf8 line"));
        }
        match p2p_protocol::read_frame(reader).await {
            Ok(frame) => reassembler.push_frame(&frame).expect("push frame"),
            Err(_) => return None,
        }
    }
}
