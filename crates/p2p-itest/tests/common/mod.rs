//! T27 共享夹具：真实双节点（helper Endpoint+门禁 / client 扮桥对端）loopback
//! transport + repair_bridge::pump 库 + mint 现铸票据 + MCP 请求封装。
//! 只放装置与有界等待（显式 timeout，无长 sleep）；断言留在 repair_e2e.rs。

use p2p::{BoxedStream, Node, PeerId, ProtocolId};
use p2p_identity::Keypair;
use p2p_protocol::write_frame;
use repair_bridge::{pump, PROTOCOL_ID};
use repair_enforce::whitelist::ShellWhitelist;
use repair_enforce::{Approver, Clock};
use repair_helper::audit::{now_unix_ms, AuditSink};
use repair_helper::jail::PathJail;
use repair_helper::p2p::{Endpoint, InboundPeers};
use repair_helper::ticket::{mint, TicketLedger, TicketVerifier, SCOPE_DIAG};
use repair_helper::tools::approval::{QueueApprover, WallClock};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::task::JoinHandle;
/// 单步等待上限：本地 loopback 全链往返毫秒级，10s 是宽松护栏。
pub const STEP: Duration = Duration::from_secs(10);
pub const INIT_V18: &str = r#"{"protocolVersion":"2025-06-18"}"#;
pub const INIT_V26: &str = r#"{"protocolVersion":"2025-03-26"}"#;
pub const SHELL_HIT: &str = r#"{"argv":["echo","hi"]}"#;
pub const SHELL_MISS: &str = r#"{"argv":["rm","-rf","/"]}"#;

/// MCP 会话：stdin 写半（发请求）+ stdout 读半（收响应）+ pump 任务。
pub struct Session {
    stdin: DuplexStream,
    stdout: BufReader<DuplexStream>,
    pump: JoinHandle<std::io::Result<()>>,
}

/// 授权根夹具：临时目录 + 一个可读文件。
pub fn fixture_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rs-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    root
}

/// 平台密钥（mint 与 verify 同源，模拟调度侧签发）。
pub fn platform() -> Keypair {
    Keypair::from_seed(&[7u8; 32])
}

/// 无预批审批通道（diag/断线用例用，write 走 scope 门即拒）。
pub fn noop_approver() -> Arc<Mutex<Box<dyn Approver + Send>>> {
    Arc::new(Mutex::new(Box::new(QueueApprover::new())))
}

/// 起真实 helper 节点：Endpoint（票据门+guarded Host）+ 连接门禁捕获对端。
pub async fn helper_node(
    tag: &str,
    root: PathBuf,
    whitelist: ShellWhitelist,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
) -> Node {
    let inbound = InboundPeers::default();
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(WallClock::new());
    let verifier = TicketVerifier::new(platform().public(), TicketLedger::default());
    let jail = PathJail::from_roots(vec![root]).unwrap();
    let endpoint = Endpoint::new(
        verifier,
        inbound.clone(),
        jail,
        AuditSink::default(),
        whitelist,
        clock,
        approver,
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("rs-e2e-{tag}-h-{}", std::process::id()));
    let node = Node::builder()
        .mdns(false)
        .data_dir(dir)
        .build()
        .await
        .unwrap();
    node.set_gate(inbound.gate());
    node.handle_protocol(Arc::new(endpoint));
    node
}

/// 桥对端节点：登记 helper TCP 地址并拨号（真实 transport 直连）。
pub async fn client_node(tag: &str, helper: &Node, helper_peer: PeerId) -> Node {
    let dir = std::env::temp_dir().join(format!("rs-e2e-{tag}-c-{}", std::process::id()));
    let node = Node::builder()
        .mdns(false)
        .data_dir(dir)
        .build()
        .await
        .unwrap();
    for addr in helper.listen_addrs() {
        if addr.contains("/t") {
            node.add_peer_address(helper_peer, &addr).unwrap();
        }
    }
    node.connect(helper_peer).await.unwrap();
    node
}

/// 现铸票据：helper_peer/bridge_peer 与两端节点身份一致（§3.3）。
pub fn fresh_ticket(id: &str, scope: &str, helper_peer: PeerId, bridge_peer: PeerId) -> String {
    let h = helper_peer.to_string();
    let b = bridge_peer.to_string();
    mint(&platform(), id, &h, &b, scope, 3600, now_unix_ms() / 1000).unwrap()
}

/// 开 /repair/mcp/1 流：首帧票据（沿 repair-bridge 开流顺序），随后 pump 对拷。
/// drop 会话 stdin 即 EOF（§3.7 桥退出）。
pub async fn open_session(client: &Node, helper_peer: PeerId, ticket: &str) -> Session {
    let protocol = ProtocolId::new(PROTOCOL_ID).unwrap();
    let mut stream: BoxedStream = client.new_stream(helper_peer, protocol).await.unwrap();
    write_frame(&mut stream, ticket.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    let (read, write) = tokio::io::split(stream);
    let (stdin_w, stdin_r) = tokio::io::duplex(1 << 20);
    let (stdout_r, stdout_w) = tokio::io::duplex(1 << 20);
    Session {
        stdin: stdin_w,
        stdout: BufReader::new(stdout_r),
        pump: tokio::spawn(pump(stdin_r, write, read, stdout_w)),
    }
}

/// 一次 MCP 请求/响应往返（显式超时，禁止无界等待）。
pub async fn rpc(s: &mut Session, id: u64, method: &str, params: &str) -> Value {
    let line = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}\n"
    );
    s.stdin.write_all(line.as_bytes()).await.unwrap();
    s.stdin.flush().await.unwrap();
    let mut buf = String::new();
    tokio::time::timeout(STEP, s.stdout.read_line(&mut buf))
        .await
        .expect("rpc response timeout")
        .unwrap();
    serde_json::from_str(&buf).unwrap()
}

/// tools/call 便捷封装。
pub async fn call_tool(s: &mut Session, id: u64, name: &str, args: &str) -> Value {
    rpc(
        s,
        id,
        "tools/call",
        &format!("{{\"name\":\"{name}\",\"arguments\":{args}}}"),
    )
    .await
}

/// 断线：EOF → pump 结束（桥非零退出语义，§3.7），节点保持存活。
pub async fn drain(s: Session) {
    drop(s.stdin);
    let _ = tokio::time::timeout(STEP, s.pump)
        .await
        .expect("pump must finish on EOF");
}

/// 关停两端节点并清理夹具目录。
pub fn teardown(helper: &Node, client: &Node, root: PathBuf) {
    helper.shutdown();
    client.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

/// 起 helper+client 并开首会话（diag + 空白名单 + 无预批）。
pub async fn rig(tag: &str, ticket_id: &str) -> (Node, Node, Session, PathBuf) {
    rig_with(
        tag,
        ticket_id,
        SCOPE_DIAG,
        ShellWhitelist::empty(),
        noop_approver(),
    )
    .await
}

/// 起 helper+client 并开首会话；参数化 scope/白名单/审批通道；日志走 p2p-log。
pub async fn rig_with(
    tag: &str,
    ticket_id: &str,
    scope: &str,
    whitelist: ShellWhitelist,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
) -> (Node, Node, Session, PathBuf) {
    let _ = p2p_log::init(Default::default()); // 幂等，仅首次生效
    let root = fixture_root(tag);
    let helper = helper_node(tag, root.clone(), whitelist, approver).await;
    let helper_peer = helper.local_peer_id();
    let client = client_node(tag, &helper, helper_peer).await;
    let ticket = fresh_ticket(ticket_id, scope, helper_peer, client.local_peer_id());
    let session = open_session(&client, helper_peer, &ticket).await;
    (helper, client, session, root)
}
