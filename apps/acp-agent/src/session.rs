//! 单条入站流的生命周期（设计 §4.1/§5/§6/§7）：
//! 握手 -> 流身份归属（底座随流下传互认 peer，无身份即 fail-closed）-> 策略授权 -> 资源门禁
//! -> 分流：无票据 = fresh spawn（cwd 监狱 + 票据签发），携票据 = 续连接管。
//! 安全改写点分模块：jail（cwd）/ mcp（剥离替换）/ permission + router（瀑布）/
//! reattach（缓存）。桥自身只做编排，不解析 ACP 语义（两个安全改写点除外）。

use std::io;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use acp_common::error::ErrorCode;
use acp_common::{parse_client_hello, ClientHello, PeerPolicy, PolicyTable, Scope, ServerHello};
use p2p::{BoxedStream, PeerId};

use crate::audit::{AuditEvent, AuditSink};
use crate::child::SlotBook;
use crate::config::AgentConfig;
use crate::conn;
use crate::gate::{ConnGate, ConnGuard, GateLimits};
use crate::pump::{read_wire_line, wire_error, write_wire_line};

/// 握手读超时：无超时则慢速流可在占坑后永久挂起。
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SessionDeps {
    pub config: AgentConfig,
    pub policy: Arc<StdRwLock<PolicyTable>>,
    pub gate: Arc<ConnGate>,
    pub slots: Arc<SlotBook>,
    pub audit: Arc<dyn AuditSink>,
}

impl SessionDeps {
    /// 装配：从配置路径加载策略表（缺失=空表默认拒绝；损坏=拒启）。
    pub fn assemble(
        config: AgentConfig,
        audit: Arc<dyn AuditSink>,
    ) -> Result<Arc<Self>, acp_common::PolicyStoreError> {
        let table = crate::policy::load(&config.policy_path())?;
        Ok(Arc::new(Self {
            policy: Arc::new(StdRwLock::new(table)),
            gate: Arc::new(ConnGate::new()),
            slots: Arc::new(SlotBook::new()),
            config,
            audit,
        }))
    }
}

/// swarm 分发入口：整条流归本函数，返回即关流。peer 为分发层随流下传的
/// 握手互认身份；None（裸流入口）无归属依据，握手前即 fail-closed 拒绝。
pub async fn serve(
    deps: Arc<SessionDeps>,
    stream: BoxedStream,
    peer: Option<PeerId>,
) -> io::Result<()> {
    run_session(deps, stream, peer)
        .await
        .inspect_err(|err| tracing::debug!(error = %err, "acp session ended"))
}

async fn run_session(
    deps: Arc<SessionDeps>,
    mut stream: BoxedStream,
    peer: Option<PeerId>,
) -> io::Result<()> {
    let peer_id = match peer {
        Some(peer) => peer.to_string(),
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
    let hello = match handshake(&mut stream).await {
        Ok(hello) => hello,
        Err(code) => return deny(&mut stream, deps.audit.as_ref(), &peer_id, code).await,
    };
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
    match hello.reattach {
        Some(ticket) => conn::reattach(deps, stream, peer_id, hello, grant, guard, ticket).await,
        None => conn::fresh(deps, stream, peer_id, hello, grant, guard).await,
    }
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
pub(crate) async fn deny(
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

/// ready 回执：fresh 签发续连票据，reattach 回带原票据（客户端确认仍有效）。
pub(crate) async fn reply_ready(
    stream: &mut BoxedStream,
    scope: Scope,
    agent: &str,
    ticket: Option<&str>,
) -> io::Result<()> {
    let hello = match ticket {
        Some(ticket) => ServerHello::ready_with_ticket(scope, agent, ticket),
        None => ServerHello::ready(scope, agent),
    };
    let line = hello.to_line().map_err(wire_error)?;
    write_wire_line(stream, line.as_bytes()).await
}
