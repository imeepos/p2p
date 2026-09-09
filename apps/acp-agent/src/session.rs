//! 单条入站流的生命周期（设计 §4.1/§5/§6/§7）：
//! 握手 -> 流身份归属（底座随流下传互认 peer，无身份即 fail-closed）-> 策略授权 -> 资源门禁
//! -> 分流：无票据 = fresh spawn（cwd 监狱 + 票据签发），携票据 = 续连接管。
//! 安全改写点分模块：jail（cwd）/ mcp（剥离替换）/ permission + router（瀑布）/
//! reattach（缓存）。桥自身只做编排，不解析 ACP 语义（两个安全改写点除外）。

use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use acp_common::error::ErrorCode;
use acp_common::{parse_client_hello, ClientHello, PeerPolicy, PolicyTable, Scope, ServerHello};
use p2p::{BoxedStream, PeerId};
use p2p_authz::Permission;

use crate::audit::{AuditEvent, AuditSink};
use crate::authz::{allows_decision, deny_detail, AuthzGate, DiskAuthz};
use crate::child::SlotBook;
use crate::config::AgentConfig;
use crate::conn;
use crate::gate::{ConnGate, ConnGuard, GateLimits};
use crate::pump::{read_wire_line, wire_error, write_wire_line};
use crate::share::{RedeemOutcome, ShareService};
use crate::workspaces::WorkspaceStore;

/// 握手读超时：无超时则慢速流可在占坑后永久挂起。
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SessionDeps {
    pub config: AgentConfig,
    pub policy: Arc<StdRwLock<PolicyTable>>,
    pub shares: Arc<ShareService>,
    pub gate: Arc<ConnGate>,
    pub slots: Arc<SlotBook>,
    pub audit: Arc<dyn AuditSink>,
    /// 工作区动态表（admin 增删实时生效；jail 与分享校验同源）。
    pub workspaces: Arc<WorkspaceStore>,
    /// authz 闸（authz-role-design §8 ACP 桥行）：准入双查的 authz 侧。
    pub authz: Arc<dyn AuthzGate>,
}

/// 装配期存储错误：策略表与分享台账任一损坏都拒启（禁止静默回退）。
#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    #[error("policy load: {0}")]
    Policy(#[from] acp_common::PolicyStoreError),
    #[error("share ledger load: {0}")]
    Shares(#[from] acp_common::ShareStoreError),
    #[error("workspace store load: {0}")]
    Workspaces(#[from] crate::workspaces::WorkspaceStoreError),
}

impl SessionDeps {
    /// 装配：从配置路径加载策略表与分享台账（缺失=空表/空账，默认拒绝；
    /// 损坏=拒启）。
    pub fn assemble(
        config: AgentConfig,
        audit: Arc<dyn AuditSink>,
    ) -> Result<Arc<Self>, AssembleError> {
        let data_dir = config.data_dir.clone();
        let table = crate::policy::load(&config.policy_path())?;
        let policy = Arc::new(StdRwLock::new(table));
        let workspaces = Arc::new(WorkspaceStore::open_for_config(&config)?);
        let shares =
            ShareService::open(&config, workspaces.clone(), policy.clone(), audit.clone())?;
        Ok(Arc::new(Self {
            policy,
            shares: Arc::new(shares),
            gate: Arc::new(ConnGate::new()),
            slots: Arc::new(SlotBook::new()),
            config,
            audit,
            workspaces,
            authz: Arc::new(DiskAuthz::new(Path::new(&data_dir))),
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
    let grant = match authorize(&deps, &peer_id, hello.token.as_deref()) {
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

/// 授权瀑布（设计 §4 + authz-role-design §8 ACP 桥行，零协议变更）：
/// ① 策略表命中走既有路径（token 忽略）；② 未命中且带 token → 分享台账兑换
/// （激活写策略表或按状态拒绝）；③ authz 双闸：非 owner 授权还须
/// check(peer, acp.session)（owner 不进 authz，红线 1；绑定缺失即拒，§9）；
/// ④ 其余 fail-closed 不回退。
fn authorize(deps: &SessionDeps, peer: &str, token: Option<&str>) -> Result<PeerPolicy, ErrorCode> {
    let grant = policy_grant_or_redeem(deps, peer, token)?;
    if grant.scope != Scope::Owner && !session_permitted(deps, peer) {
        return Err(ErrorCode::PeerNotAllowed);
    }
    Ok(grant)
}

/// 既有策略面（设计 §4 ①②）：表命中或 token 兑换，行为与切换前完全一致。
fn policy_grant_or_redeem(
    deps: &SessionDeps,
    peer: &str,
    token: Option<&str>,
) -> Result<PeerPolicy, ErrorCode> {
    let table = deps
        .policy
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(grant) = table.lookup(peer) {
        return Ok(grant.clone());
    }
    drop(table);
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return Err(ErrorCode::PeerNotAllowed);
    };
    match deps
        .shares
        .redeem(peer, token, acp_common::share::unix_now())
    {
        RedeemOutcome::Activated(grant) => Ok(grant),
        RedeemOutcome::NotMatched => Err(ErrorCode::PeerNotAllowed),
        RedeemOutcome::Denied { kind, .. } => Err(kind.error_code()),
        RedeemOutcome::Storage(err) => {
            tracing::error!(peer, error = %err, "share redeem persistence failed; denying (fail-closed)");
            Err(ErrorCode::PeerNotAllowed)
        }
    }
}

/// authz 准入闸：check(peer, acp.session)，Allow 才放行；拒绝留审计
/// （reason 码只入审计不回 wire，§12-Q5；存储故障按拒，红线 2）。
fn session_permitted(deps: &SessionDeps, peer: &str) -> bool {
    let decision = deps.authz.check(peer, Permission::ACP_SESSION);
    let permitted = allows_decision(decision);
    if !permitted {
        deps.audit.record(AuditEvent::AuthzDenied {
            peer: peer.to_owned(),
            perm: Permission::ACP_SESSION.as_str(),
            detail: deny_detail(decision),
        });
    }
    permitted
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
