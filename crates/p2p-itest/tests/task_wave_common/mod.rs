//! task wave 共享台架：宿主 rig（stub 命令注入）+ guest 拨号 + 帧收发助手。
//! stub 解析顺序：A2A_TASK_STUB 环境变量 → CARGO_TARGET_DIR →
//! apps/acp-agent/target/debug（验收命令先跑 acp-agent cargo test 即已构建）。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a2a::{Message, TaskRequest};
use acp_agent::{a2a as host_a2a, CaptureAudit};
use p2p::{Node, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use serde_json::Value;

/// 单步等待上限：本地 loopback 毫秒级，15s 为宽松护栏（card wave 同源）。
pub const STEP: Duration = Duration::from_secs(15);

pub fn stub_path() -> PathBuf {
    if let Ok(p) = std::env::var("A2A_TASK_STUB") {
        return p.into();
    }
    let mut candidates = Vec::new();
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(PathBuf::from(dir).join("debug/acp-echo-stub"));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/acp-agent/target/debug/acp-echo-stub"),
    );
    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "acp-echo-stub 未构建：先跑 cd apps/acp-agent && cargo test --no-run，或设 A2A_TASK_STUB"
    );
}

pub struct HostRig {
    pub node: Node,
    pub peer: String,
    pub agents: Arc<host_a2a::AgentStore>,
    pub audit: Arc<CaptureAudit>,
    root: PathBuf,
}

impl Drop for HostRig {
    fn drop(&mut self) {
        self.node.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub async fn host_rig(tag: &str, stub_args: &[&str]) -> HostRig {
    let root = std::env::temp_dir().join(format!("a2a-task-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("data")).expect("host tmp dir");
    let mut command = vec![stub_path().to_string_lossy().into_owned()];
    command.extend(stub_args.iter().map(|s| s.to_string()));
    let cfg = acp_agent::AgentConfig {
        data_dir: root.join("data").to_string_lossy().into_owned(),
        command,
        grace_secs: 1,
        permission_timeout_secs: 2,
        descriptor_disabled: true,
        admin_disabled: true,
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
    let grants = host_a2a::GrantStore::open(root.join("data").join(host_a2a::GRANTS_FILE))
        .expect("grant store");
    let subscribers = Arc::new(host_a2a::Subscribers::new());
    let audit = Arc::new(CaptureAudit::new());
    let ws_store = Arc::new(
        acp_agent::workspaces::WorkspaceStore::open(&[], None, cfg.paths().workspaces())
            .expect("ws store"),
    );
    let tasks = Arc::new(host_a2a::TaskService::new(
        cfg.clone(),
        agents.clone(),
        Arc::new(grants),
        ws_store,
        audit.clone(),
    ));
    let deps = Arc::new(host_a2a::A2aDeps {
        config: cfg,
        agents: agents.clone(),
        keypair: keypair.clone(),
        host_peer: keypair.peer_id().to_string(),
        subscribers,
        tasks,
        audit: audit.clone(),
    });
    node.handle_protocol(Arc::new(
        host_a2a::A2aHandler::new(deps).expect("a2a handler"),
    ));
    HostRig {
        peer: node.local_peer_id().to_string(),
        node,
        agents,
        audit,
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

pub async fn send_request(stream: &mut p2p::BoxedStream, request: &TaskRequest) {
    let bytes = serde_json::to_vec(request).expect("request encode");
    write_frame(stream, &bytes).await.expect("request write");
}

/// 读一帧原始 JSON（应答/通知混排由调用方按形状分派）。
pub async fn read_json(stream: &mut p2p::BoxedStream) -> Value {
    let bytes = tokio::time::timeout(STEP, read_frame(stream))
        .await
        .expect("read timeout")
        .expect("frame read");
    serde_json::from_slice(&bytes).expect("frame decode")
}

pub fn as_response(frame: &Value) -> (Option<&Value>, Option<&Value>) {
    (frame.get("result"), frame.get("error"))
}

pub fn is_notice(frame: &Value, method: &str) -> bool {
    frame.get("method").and_then(Value::as_str) == Some(method)
}

/// 消费至 task 终态，途中收集 status 序与 message 文本。
pub async fn drive_to_terminal(stream: &mut p2p::BoxedStream) -> (Vec<String>, Vec<String>) {
    let mut statuses = Vec::new();
    let mut texts = Vec::new();
    loop {
        let frame = read_json(stream).await;
        if is_notice(&frame, "tasks/status") {
            let state = frame
                .pointer("/params/state")
                .and_then(Value::as_str)
                .expect("status state")
                .to_owned();
            let terminal = matches!(
                state.as_str(),
                "completed" | "failed" | "cancelled" | "rejected"
            );
            statuses.push(state);
            if terminal {
                return (statuses, texts);
            }
        } else if is_notice(&frame, "tasks/message") {
            texts.push(
                frame
                    .pointer("/params/message/parts/0/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            );
        }
    }
}

pub fn create_request(agent_id: &str, text: &str, id: u64) -> TaskRequest {
    TaskRequest::create(agent_id, Message::user_text(text), id)
}

/// admin HTTP 面在 task wave 不直接用；SocketAddr 随 rig 地址族保留编译校验。
#[allow(dead_code)]
pub fn socket_addr_typed(_a: SocketAddr) {}
