//! 单条入站流的生命周期（设计 §4.1/§7，ACP2 骨架范围）：
//! 握手 -> PeerId 归属 -> 策略表（默认拒绝）-> 资源门禁 -> spawn 子进程 ->
//! ready 回执 -> ndjson<->varint 有界对拷；断流走退出阶梯（stdin EOF -> 宽限 -> SIGKILL）。
//! 桥不解析 ACP 语义（握手行除外）；cwd 监狱/MCP 剥离/权限瀑布/续连缓存属 ACP4。

use std::io::{self};
use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use acp_common::error::ErrorCode;
use acp_common::{parse_client_hello, ClientHello, PeerPolicy, PolicyTable, Scope, ServerHello};
use p2p::BoxedStream;
use tokio::io::BufReader;
use tokio::process::Child;
use uuid::Uuid;

use crate::audit::{AuditEvent, AuditSink};
use crate::config::AgentConfig;
use crate::gate::{ConnGate, ConnGuard, GateLimits};
use crate::peers::PeerBook;
use crate::pump::{
    pump_child_to_wire, pump_wire_to_child, read_wire_line, wire_error, write_wire_line,
};
use crate::subprocess;

/// 握手读超时：无超时则慢速流可在占坑后永久挂起。
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// peer 归属等待：吸收流先于 PeerConnected 记账到达的竞态窗口。
const PEER_RESOLVE_WAIT: Duration = Duration::from_secs(2);

pub struct SessionDeps {
    pub config: AgentConfig,
    pub policy: Arc<StdRwLock<PolicyTable>>,
    pub gate: Arc<ConnGate>,
    pub peers: Arc<PeerBook>,
    pub audit: Arc<dyn AuditSink>,
}

impl SessionDeps {
    /// 装配：从配置路径加载策略表（缺失=空表默认拒绝；损坏=拒启）。
    pub fn assemble(
        config: AgentConfig,
        audit: Arc<dyn AuditSink>,
        peers: Arc<PeerBook>,
    ) -> Result<Arc<Self>, acp_common::PolicyStoreError> {
        let table = crate::policy::load(&config.policy_path())?;
        Ok(Arc::new(Self {
            policy: Arc::new(StdRwLock::new(table)),
            gate: Arc::new(ConnGate::new()),
            config,
            audit,
            peers,
        }))
    }
}

/// swarm 分发入口：整条流归本函数，返回即关流。
pub async fn serve(deps: Arc<SessionDeps>, stream: BoxedStream) -> io::Result<()> {
    run_session(deps, stream)
        .await
        .inspect_err(|err| tracing::debug!(error = %err, "acp session ended"))
}

async fn run_session(deps: Arc<SessionDeps>, mut stream: BoxedStream) -> io::Result<()> {
    let hello = match handshake(&mut stream).await {
        Ok(hello) => hello,
        Err(code) => return deny(&mut stream, deps.audit.as_ref(), "unknown", code).await,
    };
    let peer = match deps.peers.resolve(PEER_RESOLVE_WAIT).await {
        Some(peer) => peer,
        None => {
            return deny(
                &mut stream,
                deps.audit.as_ref(),
                "unknown",
                ErrorCode::PeerNotAllowed,
            )
            .await;
        }
    };
    let peer_id = peer.to_string();
    let grant = match authorize(&deps, &peer_id) {
        Ok(grant) => grant,
        Err(code) => return deny(&mut stream, deps.audit.as_ref(), &peer_id, code).await,
    };
    let guard = match admit(&deps, &peer_id) {
        Ok(guard) => guard,
        Err((code, limit)) => {
            deps.audit.record(AuditEvent::GateDenied {
                peer: peer_id.clone(),
                code: code.code().to_owned(),
                limit,
            });
            return deny(&mut stream, deps.audit.as_ref(), &peer_id, code).await;
        }
    };
    drive_session(deps, &mut stream, peer_id, hello, grant, guard).await
}

async fn handshake(stream: &mut BoxedStream) -> Result<ClientHello, ErrorCode> {
    let read = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_wire_line(stream)).await;
    let line = match read {
        Ok(Ok(line)) => line,
        Ok(Err(err)) => {
            tracing::debug!(error = %err, "handshake read failed");
            return Err(ErrorCode::HandshakeMalformed);
        }
        Err(_) => {
            tracing::debug!("handshake timed out");
            return Err(ErrorCode::HandshakeMalformed);
        }
    };
    let text = std::str::from_utf8(&line).map_err(|_| ErrorCode::HandshakeMalformed)?;
    parse_client_hello(text).map_err(|_| ErrorCode::HandshakeMalformed)
}

fn authorize(deps: &SessionDeps, peer: &str) -> Result<PeerPolicy, ErrorCode> {
    let table = deps
        .policy
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.authorize(peer).cloned()
}

fn admit(deps: &SessionDeps, peer: &str) -> Result<ConnGuard, (ErrorCode, &'static str)> {
    let limits = GateLimits::from_config(deps.config.max_connections);
    ConnGuard::acquire(deps.gate.clone(), peer, limits)
}

/// 拒绝路径（设计 §12-Q5）：denied 帧只带码，审计只记 PeerId+码+时间。
async fn deny(
    stream: &mut BoxedStream,
    audit: &dyn AuditSink,
    peer: &str,
    code: ErrorCode,
) -> io::Result<()> {
    let hello = ServerHello::Denied {
        denied: code.code().to_owned(),
    };
    if let Ok(line) = hello.to_line() {
        let _ = write_wire_line(stream, line.as_bytes()).await;
    }
    audit.record(AuditEvent::ConnDenied {
        peer: peer.to_owned(),
        code: code.code().to_owned(),
    });
    Err(io::Error::other(format!("connection denied: {code}")))
}

async fn drive_session(
    deps: Arc<SessionDeps>,
    stream: &mut BoxedStream,
    peer_id: String,
    hello: ClientHello,
    grant: PeerPolicy,
    _guard: ConnGuard,
) -> io::Result<()> {
    let log_path = stderr_log_path(&deps, &peer_id, hello.conn);
    let sub = match subprocess::spawn(&deps.config.command, log_path) {
        Ok(sub) => sub,
        Err(err) => {
            deps.audit.record(AuditEvent::SpawnFailed {
                peer: peer_id.clone(),
                conn: hello.conn.to_string(),
                detail: err.to_string(),
            });
            return deny(
                stream,
                deps.audit.as_ref(),
                &peer_id,
                ErrorCode::SubprocessFailed,
            )
            .await;
        }
    };
    reply_ready(stream, grant.scope, &deps.config.agent_name).await?;
    deps.audit.record(AuditEvent::ConnEstablished {
        peer: peer_id.clone(),
        conn: hello.conn.to_string(),
    });
    let parts: SubprocessParts = sub.into();
    let SubprocessParts {
        child,
        mut stdin,
        stdout,
    } = parts;
    let mut child_out = BufReader::new(stdout);
    let (mut rx, mut tx) = tokio::io::split(stream);
    let side = tokio::select! {
        res = pump_child_to_wire(&mut child_out, &mut tx) => PumpSide::Child(res),
        res = pump_wire_to_child(&mut rx, &mut stdin) => PumpSide::Client(res),
    };
    // 泵结束即双方未来被丢弃：stdin 落体 = 子进程收到 EOF（干净退出机会）
    finish_subprocess(&deps, &peer_id, &hello, child, side).await
}

struct SubprocessParts {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
}

impl From<subprocess::Subprocess> for SubprocessParts {
    fn from(sub: subprocess::Subprocess) -> Self {
        Self {
            child: sub.child,
            stdin: sub.stdin,
            stdout: sub.stdout,
        }
    }
}

enum PumpSide {
    Child(io::Result<()>),
    Client(io::Result<()>),
}

/// 退出阶梯收尾：stdin 已 EOF，宽限内退出则记录状态，超时 SIGKILL；
/// 泵的 Err（护栏击穿/传输异常）最终上抛留可观测信号，不静默。
async fn finish_subprocess(
    deps: &SessionDeps,
    peer_id: &str,
    hello: &ClientHello,
    child: Child,
    side: PumpSide,
) -> io::Result<()> {
    let mut breach: Option<String> = None;
    match &side {
        PumpSide::Child(Ok(())) => {}
        PumpSide::Child(Err(err)) => breach = Some(format!("child-to-wire pump failed: {err}")),
        PumpSide::Client(Ok(())) => deps.audit.record(AuditEvent::ClientGone {
            peer: peer_id.to_owned(),
            conn: hello.conn.to_string(),
        }),
        PumpSide::Client(Err(err)) => breach = Some(format!("wire-to-child pump failed: {err}")),
    }
    let detail = reap(child, deps.config.grace()).await;
    deps.audit.record(AuditEvent::SubprocessExit {
        peer: peer_id.to_owned(),
        conn: hello.conn.to_string(),
        detail: detail.clone(),
    });
    match breach {
        Some(why) => Err(io::Error::other(why)),
        None => Ok(()),
    }
}

/// 宽限等待子进程退出；超时 SIGKILL（退出阶梯末级）。
async fn reap(mut child: Child, grace: Duration) -> String {
    match tokio::time::timeout(grace, child.wait()).await {
        Ok(Ok(status)) => format!("exit {status}"),
        Ok(Err(err)) => format!("wait failed: {err}"),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            "killed after grace".to_owned()
        }
    }
}

async fn reply_ready(stream: &mut BoxedStream, scope: Scope, agent: &str) -> io::Result<()> {
    let hello = ServerHello::ready(scope, agent);
    let line = hello.to_line().map_err(wire_error)?;
    write_wire_line(stream, line.as_bytes()).await
}

fn stderr_log_path(deps: &SessionDeps, peer: &str, conn: Uuid) -> PathBuf {
    deps.config
        .log_dir()
        .join(format!("{peer}-{}.log", conn.simple()))
}
