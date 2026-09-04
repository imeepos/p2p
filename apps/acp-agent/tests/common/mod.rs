#![allow(dead_code)]

//! 回环测试共享设施：同进程双 facade 节点（服务端挂 acp-agent handler，
//! 客户端为测试拨号方），子进程用 acp-echo-stub 桩（CARGO_BIN_EXE 定位）。

use std::path::PathBuf;
use std::sync::Arc;

use acp_agent::{AcpHandler, AgentConfig, CaptureAudit, PeerBook, SessionDeps};
use acp_common::{
    frames, AskRoute, ClientHello, LineReassembler, PeerPolicy, PolicyTable, Scope, ServerHello,
};
use p2p::{BoxedStream, Node, PeerId, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

pub const STUB: &str = env!("CARGO_BIN_EXE_acp-echo-stub");
pub const PROTO: &str = "/dsh-acp/1";

pub fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-agent-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir
}

pub fn test_config(tag: &str) -> AgentConfig {
    AgentConfig {
        data_dir: tmp_dir(tag).to_string_lossy().into_owned(),
        command: vec![STUB.to_owned()],
        grace_secs: 1,
        // ACP4：断流进续连窗口；测试统一缩到 1s，窗口过期即走退出阶梯
        reattach_window_secs: 1,
        permission_timeout_secs: 2,
        ..AgentConfig::default()
    }
}

fn test_grant(scope: Scope) -> PeerPolicy {
    PeerPolicy {
        scope,
        allow_mcp: Vec::new(),
        ask_route: AskRoute::RemoteGui,
        note: String::new(),
        granted_at: "2026-01-01T00:00:00Z".to_owned(),
        fingerprint: "itest".to_owned(),
    }
}

/// 先建空表或授予单 peer，再 build_server（策略表只在装配期加载一次）。
pub fn write_policy(cfg: &AgentConfig, grant: Option<(&PeerId, Scope)>) {
    let path = cfg.policy_path();
    let mut table = PolicyTable::new();
    if let Some((peer, scope)) = grant {
        table.grant(peer.to_string(), test_grant(scope));
    }
    std::fs::create_dir_all(path.parent().expect("policy parent")).expect("mkdir");
    table.save(&path).expect("save policy");
}

pub async fn build_server(cfg: &AgentConfig) -> (Node, Arc<CaptureAudit>) {
    let (node, audit, _deps) = build_server_full(cfg).await;
    (node, audit)
}

/// 带 SessionDeps 的变体：退出阶梯等需要触达槽位簿记的测试使用。
pub async fn build_server_full(cfg: &AgentConfig) -> (Node, Arc<CaptureAudit>, Arc<SessionDeps>) {
    let node = Node::builder()
        .mdns(false)
        .data_dir(cfg.paths().root.join("identity"))
        .build()
        .await
        .expect("server node");
    let audit = Arc::new(CaptureAudit::new());
    let peers = PeerBook::spawn(node.events());
    let deps = SessionDeps::assemble(cfg.clone(), audit.clone(), peers).expect("deps");
    node.handle_protocol(Arc::new(AcpHandler::new(deps.clone()).expect("handler")));
    (node, audit, deps)
}

pub async fn build_client(tag: &str) -> Node {
    Node::builder()
        .mdns(false)
        .data_dir(tmp_dir(&format!("{tag}-client")))
        .build()
        .await
        .expect("client node")
}

/// 只登记 QUIC 地址：yamux 空闲连接二次 open_stream 有上游缺陷（facade.rs 已记），
/// 超上限用例需要同连接多流，故统一走 QUIC 原生多流。
pub fn seed_quic(server: &Node, server_peer: PeerId, client: &Node) {
    for addr in server.listen_addrs() {
        if addr.contains("/u") {
            client
                .add_peer_address(server_peer, &addr)
                .expect("seed addr");
        }
    }
}

pub async fn connect_and_stream(client: &Node, server_peer: PeerId) -> BoxedStream {
    client.connect(server_peer).await.expect("connect");
    client
        .new_stream(
            server_peer,
            ProtocolId::new(PROTO).expect("valid protocol id"),
        )
        .await
        .expect("open acp stream")
}

pub async fn send_line<W: AsyncWrite + Unpin + Send>(writer: &mut W, line: &str) {
    for frame in frames(line.as_bytes()) {
        write_frame(writer, frame).await.expect("write frame");
    }
    writer.flush().await.expect("flush");
}

/// 读一条 ndjson 行；流关闭返回 None。
pub async fn read_line<R: AsyncRead + Unpin + Send>(reader: &mut R) -> Option<String> {
    let mut reassembler = LineReassembler::new();
    loop {
        if let Some(line) = reassembler.take_line() {
            return Some(String::from_utf8(line).expect("utf8 line"));
        }
        match read_frame(reader).await {
            Ok(frame) => reassembler.push_frame(&frame).expect("push frame"),
            Err(_) => return None,
        }
    }
}

/// 客户端握手：发 ClientHello 行，收 ready/denied 回执。
pub async fn handshake_client(stream: &mut BoxedStream) -> ServerHello {
    let hello = ClientHello::new(Uuid::new_v4());
    let line = hello.to_line().expect("hello line");
    send_line(stream, &line).await;
    let reply = read_line(stream)
        .await
        .expect("server handshake reply")
        .trim_end()
        .to_owned();
    acp_common::parse_server_hello(&reply).expect("parse server hello")
}
pub fn test_grant_full(scope: Scope, allow_mcp: Vec<String>, ask_route: AskRoute) -> PeerPolicy {
    PeerPolicy {
        allow_mcp,
        ask_route,
        ..test_grant(scope)
    }
}

/// 策略表写入（扩展形态）：条目自带 allow_mcp / ask_route。
pub fn write_policy_full(cfg: &AgentConfig, grant: Option<(&PeerId, PeerPolicy)>) {
    let path = cfg.policy_path();
    let mut table = PolicyTable::new();
    if let Some((peer, policy)) = grant {
        table.grant(peer.to_string(), policy);
    }
    std::fs::create_dir_all(path.parent().expect("policy parent")).expect("mkdir");
    table.save(&path).expect("save policy");
}

/// 续连握手：携票据回连。
pub async fn handshake_client_reattach(stream: &mut BoxedStream, ticket: Uuid) -> ServerHello {
    let mut hello = ClientHello::new(Uuid::new_v4());
    hello.reattach = Some(ticket);
    let line = hello.to_line().expect("hello line");
    send_line(stream, &line).await;
    let reply = read_line(stream)
        .await
        .expect("server handshake reply")
        .trim_end()
        .to_owned();
    acp_common::parse_server_hello(&reply).expect("parse server hello")
}

/// session/new 请求行（带 mcpServers 载荷）。
pub fn session_new_with_mcp(id: i64, mcp: serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/new",
        "params": { "cwd": "/tmp", "mcpServers": mcp },
    })
    .to_string()
}

/// 子进程侧权限请求行（经回声桩注入 child->wire 方向）。
pub fn permission_request(id: i64, kind: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/request_permission",
        "params": {
            "toolCall": { "kind": kind, "title": "itest" },
            "options": [
                { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
                { "optionId": "reject-once", "name": "Deny", "kind": "reject_once" },
            ],
            "sessionId": "s1",
        },
    })
    .to_string()
}

/// 客户端对权限请求的批准应答行。
pub fn permission_approve(id: i64, option: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "outcome": { "outcome": "selected", "optionId": option } },
    })
    .to_string()
}

/// 测试台架：客户端节点 + 挂 handler 的服务端节点 + 审计捕获 + QUIC 地址播种。
pub struct Rig {
    pub server: Node,
    pub audit: std::sync::Arc<CaptureAudit>,
    pub server_peer: PeerId,
    pub client: Node,
}

pub async fn rig(tag: &str, grant: PeerPolicy, tweak: impl FnOnce(&mut AgentConfig)) -> Rig {
    let client = build_client(tag).await;
    let mut cfg = test_config(tag);
    tweak(&mut cfg);
    write_policy_full(&cfg, Some((&client.local_peer_id(), grant)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);
    Rig {
        server,
        audit,
        server_peer,
        client,
    }
}

pub fn shutdown(rig: &Rig) {
    rig.server.shutdown();
    rig.client.shutdown();
}
/// 连接服务端并打开 /dsh-acp/1 流。
pub async fn open_stream(rig: &Rig) -> BoxedStream {
    rig.client.connect(rig.server_peer).await.expect("connect");
    rig.client
        .new_stream(
            rig.server_peer,
            ProtocolId::new(PROTO).expect("valid protocol id"),
        )
        .await
        .expect("open acp stream")
}
