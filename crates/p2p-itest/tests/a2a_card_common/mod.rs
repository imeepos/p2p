//! card wave 共享台架：宿主 rig（agent 簿 + task 服务 + admin HTTP）+ guest 拨号
//! + card 相帧收发助手 + admin HTTP 助手。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a2a::CardFrame;
use acp_agent::share::admin::{AdminDeps, AdminServer, AdminToken};
use acp_agent::share::LinkContext;
use acp_agent::{a2a as host_a2a, CaptureAudit};
use p2p::{Node, ProtocolId};
use p2p_protocol::{read_frame, write_frame};

/// 单步等待上限：本地 loopback 毫秒级，15s 为宽松护栏（share_link 同源）。
pub const STEP: Duration = Duration::from_secs(15);

pub struct HostRig {
    pub node: Node,
    pub peer: String,
    pub agents: Arc<host_a2a::AgentStore>,
    pub admin_addr: SocketAddr,
    pub admin_token: String,
    root: PathBuf,
}

impl Drop for HostRig {
    fn drop(&mut self) {
        self.node.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub async fn host_rig(tag: &str) -> HostRig {
    let root = std::env::temp_dir().join(format!("a2a-card-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("data")).expect("host tmp dir");
    let cfg = acp_agent::AgentConfig {
        data_dir: root.join("data").to_string_lossy().into_owned(),
        ..acp_agent::AgentConfig::default()
    };
    let node = Node::builder()
        .mdns(false)
        .data_dir(root.join("identity"))
        .build()
        .await
        .expect("host node");
    let keypair = p2p_identity::load_seed(&root.join("identity/key.seed")).expect("host keypair");
    let agents = host_a2a::AgentStore::open(root.join("data").join(host_a2a::AGENTS_FILE))
        .expect("agent store");
    let grants = Arc::new(
        host_a2a::GrantStore::open(root.join("data").join(host_a2a::GRANTS_FILE))
            .expect("grant store"),
    );
    let subscribers = Arc::new(host_a2a::Subscribers::new());
    let audit = Arc::new(CaptureAudit::new());
    let ws_store = Arc::new(
        acp_agent::workspaces::WorkspaceStore::open(&[], None, cfg.paths().workspaces())
            .expect("ws store"),
    );
    let tasks = Arc::new(host_a2a::TaskService::new(
        cfg.clone(),
        agents.clone(),
        grants,
        ws_store.clone(),
        audit.clone(),
    ));
    let deps = Arc::new(host_a2a::A2aDeps {
        config: cfg.clone(),
        agents: agents.clone(),
        keypair: keypair.clone(),
        host_peer: keypair.peer_id().to_string(),
        subscribers: subscribers.clone(),
        tasks,
        audit,
    });
    node.handle_protocol(Arc::new(
        host_a2a::A2aHandler::new(deps.clone()).expect("a2a handler"),
    ));
    // 真 admin HTTP（Bearer + loopback），a2a 管理上下文挂入
    let token = AdminToken::issue(cfg.paths().admin_token()).expect("admin token");
    let policy_path = cfg.policy_path();
    std::fs::create_dir_all(policy_path.parent().expect("policy parent")).expect("mkdir policy");
    let policy = Arc::new(std::sync::RwLock::new(acp_common::PolicyTable::new()));
    let service = acp_agent::ShareService::open(
        &cfg,
        ws_store.clone(),
        policy,
        Arc::new(CaptureAudit::new()),
    )
    .expect("service");
    let server = AdminServer::start(
        0,
        token.value.clone(),
        AdminDeps {
            service: Arc::new(service),
            link: LinkContext {
                peer: node.local_peer_id().to_string(),
                addrs: node.listen_addrs(),
            },
            workspaces: ws_store,
            a2a_admin: Some(Arc::new(host_a2a::A2aAdminCtx {
                agents: agents.clone(),
                subscribers: subscribers.clone(),
                keypair: keypair.clone(),
                host_peer: keypair.peer_id().to_string(),
            })),
        },
    )
    .await
    .expect("admin server");
    HostRig {
        peer: node.local_peer_id().to_string(),
        node,
        agents,
        admin_addr: server.addr,
        admin_token: token.value,
        root,
    }
}

pub fn parse_peer(s: &str) -> p2p::PeerId {
    let raw: [u8; 32] = bs58::decode(s)
        .into_vec()
        .expect("peer base58")
        .try_into()
        .expect("peer len");
    p2p::PeerId::from_bytes(raw)
}

/// 访客节点：登记宿主地址后直连，开 /a2a/1 card 相流。
pub async fn guest_stream(
    guest: &Node,
    host_peer: &str,
    host_addrs: &[String],
) -> p2p::BoxedStream {
    let peer = parse_peer(host_peer);
    for addr in host_addrs {
        guest.add_peer_address(peer, addr).expect("add addr");
    }
    guest.connect(peer).await.expect("guest connect");
    let protocol = ProtocolId::new(a2a::PROTOCOL_ID).expect("protocol id");
    guest.new_stream(peer, protocol).await.expect("a2a stream")
}

pub async fn send_frame(stream: &mut p2p::BoxedStream, frame: &CardFrame) {
    let bytes = serde_json::to_vec(frame).expect("frame encode");
    write_frame(stream, &bytes).await.expect("frame write");
}

pub async fn read_frame_json(stream: &mut p2p::BoxedStream) -> CardFrame {
    let bytes = tokio::time::timeout(STEP, read_frame(stream))
        .await
        .expect("read timeout")
        .expect("frame read");
    serde_json::from_slice(&bytes).expect("frame decode")
}

/// admin DELETE /a2a/agents/{id}（share_link support 的 admin_call 同款手写 HTTP）。
pub async fn admin_delete(addr: SocketAddr, token: &str, agent_id: &str) -> (u16, String) {
    let mut tcp = tokio::net::TcpStream::connect(addr)
        .await
        .expect("admin connect");
    let req = format!(
        "DELETE /a2a/agents/{agent_id} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    tcp.write_all(req.as_bytes()).await.expect("admin write");
    let mut raw = Vec::new();
    tokio::time::timeout(STEP, tcp.read_to_end(&mut raw))
        .await
        .expect("admin read timeout")
        .expect("admin read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}
