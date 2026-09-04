//! 连接路径（设计 §5/§6）：fresh = 顶替遗留槽位 + cwd 监狱 + spawn + 票据签发；
//! reattach = 票据校验 + 槽位接管 + initialize 过桥后补放；公共尾巴 attach_and_pump
//! 是 wire 双向泵：下行 mcpServers 改写（拒绝路径回 JSON-RPC 错误），上行经 router
//! 做权限瀑布与续连缓存。

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use acp_common::error::ErrorCode;
use acp_common::{ClientHello, PeerPolicy, Scope};
use p2p::BoxedStream;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::child::{self, Ctl, DetachGuard, SlotHandle};
use crate::jail;
use crate::mcp;
use crate::pump::{read_wire_line, write_wire_line};
use crate::session::{deny, reply_ready, SessionDeps};

/// 输出面通道容量：wire 帧背压已限速，64 行缓冲吸收抖动。
const SINK_CAP: usize = 64;
/// writer 收尾超时：Detach 后 sender 归零即退出，超时仅防异常滞留。
const WRITER_DRAIN: Duration = Duration::from_secs(2);

/// 新连接：无票据重连视为放弃续连，先顶替同 peer 遗留窗口槽位（孤儿进程不过夜）。
pub(crate) async fn fresh(
    deps: Arc<SessionDeps>,
    mut stream: BoxedStream,
    peer_id: String,
    hello: ClientHello,
    grant: PeerPolicy,
    _guard: crate::gate::ConnGuard,
) -> io::Result<()> {
    audit_supersede(&deps, &peer_id).await;
    let cwd = match jail::resolve(&deps.config, grant.scope, &peer_id) {
        Ok(cwd) => cwd,
        Err(err) => {
            deps.audit.record(AuditEvent::CwdDenied {
                peer: peer_id.clone(),
                scope: scope_label(grant.scope),
                detail: err.to_string(),
            });
            return deny(
                &mut stream,
                deps.audit.as_ref(),
                &peer_id,
                ErrorCode::CwdDenied,
            )
            .await;
        }
    };
    let stderr_log = stderr_log_path(&deps, &peer_id, hello.conn);
    let spawned = child::spawn_slot(
        child::SpawnCtx {
            config: deps.config.clone(),
            audit: deps.audit.clone(),
            book: deps.slots.clone(),
            peer_id: peer_id.clone(),
            conn: hello.conn.to_string(),
            grant: grant.clone(),
        },
        cwd,
        stderr_log,
    );
    let handle = match spawned {
        Ok(handle) => handle,
        Err(err) => {
            deps.audit.record(AuditEvent::SpawnFailed {
                peer: peer_id.clone(),
                conn: hello.conn.to_string(),
                detail: err.to_string(),
            });
            return deny(
                &mut stream,
                deps.audit.as_ref(),
                &peer_id,
                ErrorCode::SubprocessFailed,
            )
            .await;
        }
    };
    let ticket = handle.ticket.to_string();
    reply_ready(
        &mut stream,
        grant.scope,
        &deps.config.agent_name,
        Some(&ticket),
    )
    .await?;
    audit_established(&deps, &peer_id, &hello).await;
    attach_and_pump(deps, stream, peer_id, hello, grant, handle, false).await
}

/// 续连：同 PeerId 携票据接管活槽位；initialize 过桥后由 router 补放缓存。
pub(crate) async fn reattach(
    deps: Arc<SessionDeps>,
    mut stream: BoxedStream,
    peer_id: String,
    hello: ClientHello,
    grant: PeerPolicy,
    _guard: crate::gate::ConnGuard,
    ticket: Uuid,
) -> io::Result<()> {
    let handle = match deps.slots.validate(&ticket, &peer_id) {
        Some(handle) => handle,
        None => {
            deps.audit.record(AuditEvent::ReattachDenied {
                peer: peer_id.clone(),
                detail: format!("ticket={ticket}"),
            });
            return deny(
                &mut stream,
                deps.audit.as_ref(),
                &peer_id,
                ErrorCode::ReattachTicketInvalid,
            )
            .await;
        }
    };
    let ticket = handle.ticket.to_string();
    reply_ready(
        &mut stream,
        grant.scope,
        &deps.config.agent_name,
        Some(&ticket),
    )
    .await?;
    audit_established(&deps, &peer_id, &hello).await;
    attach_and_pump(deps, stream, peer_id, hello, grant, handle, true).await
}
async fn audit_supersede(deps: &SessionDeps, peer_id: &str) {
    let superseded = deps.slots.supersede(peer_id);
    if superseded > 0 {
        deps.audit.record(AuditEvent::SlotSuperseded {
            peer: peer_id.to_owned(),
            detail: format!("slots={superseded}"),
        });
    }
}

async fn audit_established(deps: &SessionDeps, peer_id: &str, hello: &ClientHello) {
    deps.audit.record(AuditEvent::ConnEstablished {
        peer: peer_id.to_owned(),
        conn: hello.conn.to_string(),
    });
}

/// 公共尾巴：接管输出面（writer 任务单写者），断流即 Detach 进窗口。
/// fresh defer=false（无缓存可补）；reattach defer=true（initialize 后补放）。
async fn attach_and_pump(
    deps: Arc<SessionDeps>,
    stream: BoxedStream,
    peer_id: String,
    hello: ClientHello,
    grant: PeerPolicy,
    handle: SlotHandle,
    defer: bool,
) -> io::Result<()> {
    let (sink_tx, sink_rx) = mpsc::channel(SINK_CAP);
    let (mut wire_rx, wire_tx) = tokio::io::split(stream);
    let writer = tokio::spawn(writer_task(sink_rx, wire_tx));
    let attach = Ctl::Attach {
        sink: sink_tx.clone(),
        defer,
        conn: hello.conn.to_string(),
    };
    if handle.ctl.send(attach).await.is_err() {
        drop(sink_tx);
        let _ = writer.await;
        return Err(io::Error::other("subprocess slot closed during attach"));
    }
    let outcome = {
        let _detach = DetachGuard::new(handle.ctl.clone());
        pump_loop(
            &deps,
            &mut wire_rx,
            &sink_tx,
            &handle,
            &peer_id,
            &hello,
            &grant,
        )
        .await
    };
    drop(sink_tx);
    let _ = tokio::time::timeout(WRITER_DRAIN, writer).await;
    outcome
}

/// wire -> child 下行泵：session/new 过 mcpServers 安全改写点；其余原样入槽。
/// 客户端 EOF = ClientGone（Detach 由守卫触发）；护栏击穿上抛断流。
async fn pump_loop(
    deps: &SessionDeps,
    wire_rx: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    sink_tx: &mpsc::Sender<Vec<u8>>,
    handle: &SlotHandle,
    peer_id: &str,
    hello: &ClientHello,
    grant: &PeerPolicy,
) -> io::Result<()> {
    loop {
        let line = match read_wire_line(wire_rx).await {
            Ok(line) => line,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
                deps.audit.record(AuditEvent::ClientGone {
                    peer: peer_id.to_owned(),
                    conn: hello.conn.to_string(),
                });
                return Ok(());
            }
            Err(err) => return Err(err),
        };
        let verdict = mcp::rewrite(&line, &grant.allow_mcp, &deps.config.mcp_definitions);
        if verdict.action != "untouched" {
            deps.audit.record(AuditEvent::McpRewritten {
                peer: peer_id.to_owned(),
                conn: hello.conn.to_string(),
                action: verdict.action.to_owned(),
                detail: verdict.detail.clone(),
            });
        }
        if let Some(child_line) = verdict.child_line {
            if handle.ctl.send(Ctl::ToChild(child_line)).await.is_err() {
                return Err(io::Error::other("subprocess slot closed"));
            }
        }
        if let Some(err_line) = verdict.wire_error {
            if sink_tx.send(err_line.into_bytes()).await.is_err() {
                return Err(io::Error::other("wire sink closed"));
            }
        }
    }
}

/// 输出面单写者：router（透传/补放/代答）与会话层（mcp 拒绝应答）都经此通道，
/// 通道序即 wire 序。写失败即断流留日志，不静默。
async fn writer_task(
    mut sink_rx: mpsc::Receiver<Vec<u8>>,
    mut wire_tx: impl tokio::io::AsyncWrite + Unpin + Send,
) {
    while let Some(line) = sink_rx.recv().await {
        if write_wire_line(&mut wire_tx, &line).await.is_err() {
            tracing::warn!("wire sink write failed; dropping rest of output");
            return;
        }
    }
}

fn stderr_log_path(deps: &SessionDeps, peer: &str, conn: Uuid) -> PathBuf {
    deps.config
        .log_dir()
        .join(format!("{peer}-{}.log", conn.simple()))
}

fn scope_label(scope: Scope) -> String {
    match scope {
        Scope::Owner => "owner",
        Scope::Sandbox => "sandbox",
        Scope::Workspace => "workspace",
    }
    .to_owned()
}
