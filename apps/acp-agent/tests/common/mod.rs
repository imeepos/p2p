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
    let node = Node::builder()
        .mdns(false)
        .data_dir(cfg.paths().root.join("identity"))
        .build()
        .await
        .expect("server node");
    let audit = Arc::new(CaptureAudit::new());
    let peers = PeerBook::spawn(node.events());
    let deps = SessionDeps::assemble(cfg.clone(), audit.clone(), peers).expect("deps");
    node.handle_protocol(Arc::new(AcpHandler::new(deps).expect("handler")));
    (node, audit)
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
