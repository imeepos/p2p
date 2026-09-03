//! T26 p2p 端点 in-process 集成测试：协议 ID -> 票据 -> guarded Host 全链
//! （不起真实网络，桥对端用内存 duplex 扮演，经 LoopbackHub + dispatch_inbound
//! 复刻真实节点收流路径）；票据矩阵另见 ticket_tests.rs。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use p2p_identity::{Keypair, PeerId};
use p2p_protocol::{
    dispatch_inbound, open_with_protocol, read_frame, write_frame, HandlerRegistry, LoopbackHub,
    ProtocolId, StreamFactory,
};
use repair_bridge::PROTOCOL_ID;
use repair_enforce::approval::{Approver, Clock};
use repair_enforce::whitelist::ShellWhitelist;

use crate::audit::AuditSink;
use crate::jail::PathJail;
use crate::p2p::{Endpoint, InboundPeers};
use crate::ticket::{mint, TicketLedger, TicketVerifier, SCOPE_DIAG};
use crate::tools::approval::{QueueApprover, WallClock};

fn platform() -> Keypair {
    Keypair::from_seed(&[7u8; 32])
}

fn helper_peer() -> PeerId {
    Keypair::from_seed(&[1u8; 32]).peer_id()
}

fn bridge_peer() -> PeerId {
    Keypair::from_seed(&[2u8; 32]).peer_id()
}

fn other_peer() -> PeerId {
    Keypair::from_seed(&[3u8; 32]).peer_id()
}

fn fixture_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rh-p2p-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    root
}

fn new_endpoint(peers: InboundPeers, ledger: TicketLedger, audit: AuditSink) -> Endpoint {
    let jail = PathJail::from_roots(vec![fixture_root("ep")]).unwrap();
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(WallClock::new());
    let approver: Arc<Mutex<Box<dyn Approver + Send>>> = Arc::new(Mutex::new(Box::new(
        QueueApprover::new(),
    )
        as Box<dyn Approver + Send>));
    Endpoint::new(
        TicketVerifier::new(platform().public(), ledger),
        peers,
        jail,
        audit,
        ShellWhitelist::empty(),
        clock,
        approver,
    )
    .unwrap()
}

fn registry_for(endpoint: Endpoint) -> HandlerRegistry {
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(endpoint));
    registry
}

fn fresh_ticket(id: &str, now: u64) -> String {
    mint(
        &platform(),
        id,
        &helper_peer().to_string(),
        &bridge_peer().to_string(),
        SCOPE_DIAG,
        3600,
        now,
    )
    .unwrap()
}

/// 桥对端：开流并写协议 ID + 首帧票据（沿 repair-bridge 开流顺序）。
async fn open_bridge_stream(hub: &LoopbackHub, ticket: &str) -> p2p::BoxedStream {
    let protocol = ProtocolId::new(PROTOCOL_ID).unwrap();
    let mut client = hub.open_stream(&bridge_peer(), &protocol).await.unwrap();
    client = open_with_protocol(client, &protocol).await.unwrap();
    write_frame(&mut client, ticket.as_bytes()).await.unwrap();
    client
}

#[tokio::test]
async fn full_chain_serves_mcp_over_framed_stream() {
    let now = crate::audit::now_unix_ms() / 1000;
    let peers = InboundPeers::default();
    peers.record(bridge_peer());
    let audit_path =
        std::env::temp_dir().join(format!("rh-p2p-audit-{}-chain.jsonl", std::process::id()));
    let audit = AuditSink::with_file(&audit_path).unwrap();
    let endpoint = new_endpoint(peers, TicketLedger::default(), audit.clone());
    let registry = registry_for(endpoint);

    let (hub, mut inbound) = LoopbackHub::new(8, 1 << 20);
    let ticket = fresh_ticket("t-e2e", now);
    let mut client = open_bridge_stream(&hub, &ticket).await;
    let server = inbound.recv().await.unwrap();
    let served = tokio::spawn(async move { dispatch_inbound(server, &registry).await });

    // initialize：版本协商
    write_frame(
        &mut client,
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\"}}\n",
    )
    .await
    .unwrap();
    let resp = String::from_utf8(read_frame(&mut client).await.unwrap()).unwrap();
    assert!(resp.contains("protocolVersion"), "{resp}");
    assert!(resp.contains("repair-helper"), "{resp}");

    // tools/list：含 session_report
    write_frame(
        &mut client,
        b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n",
    )
    .await
    .unwrap();
    let resp = String::from_utf8(read_frame(&mut client).await.unwrap()).unwrap();
    assert!(resp.contains("session_report"), "{resp}");

    // fs_read：走 guarded Host 全链路
    write_frame(
        &mut client,
        b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"fs_read\",\"arguments\":{\"path\":\"a.txt\"}}}\n",
    )
    .await
    .unwrap();
    let resp = String::from_utf8(read_frame(&mut client).await.unwrap()).unwrap();
    assert!(resp.contains("\"text\":\"hello\""), "{resp}");

    // session_report：内容断言（ticketId/事件/计数，read 档）
    write_frame(
        &mut client,
        b"{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"session_report\",\"arguments\":{}}}\n",
    )
    .await
    .unwrap();
    let resp = String::from_utf8(read_frame(&mut client).await.unwrap()).unwrap();
    let rpc: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let text = rpc["result"]["content"][0]["text"].as_str().unwrap();
    let report: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(report["ticketId"], "t-e2e", "{text}");
    assert_eq!(report["count"], 1, "{text}");
    assert_eq!(report["events"][0]["tool"], "fs_read", "{text}");
    assert_eq!(report["events"][0]["outcome"], "ok", "{text}");

    // 断线：流断即受理结束（§3.7）
    drop(client);
    served.await.unwrap().unwrap();

    // JSONL 落盘可读回：fs_read + session_report 两行
    let content = std::fs::read_to_string(&audit_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "audit jsonl: {content}");
    assert!(lines[0].contains("\"tool\":\"fs_read\""), "{}", lines[0]);
}

#[tokio::test]
async fn bad_signature_ticket_rejects_without_serving() {
    let now = crate::audit::now_unix_ms() / 1000;
    let peers = InboundPeers::default();
    peers.record(bridge_peer());
    let registry = registry_for(new_endpoint(
        peers,
        TicketLedger::default(),
        AuditSink::default(),
    ));
    let (hub, mut inbound) = LoopbackHub::new(8, 1 << 20);
    let ticket = fresh_ticket("t-bad", now);
    let (body, sig) = ticket.split_once('.').unwrap();
    // 中部翻转一个 base64url 字符（末位受低位约束不可改，详见 ticket_tests）
    let idx = sig.len() / 2;
    let flipped = if sig.as_bytes()[idx] == b'A' {
        'B'
    } else {
        'A'
    };
    let tampered = format!("{body}.{}{}{}", &sig[..idx], flipped, &sig[idx + 1..]);
    let mut client = open_bridge_stream(&hub, &tampered).await;
    let server = inbound.recv().await.unwrap();
    let served = tokio::spawn(async move { dispatch_inbound(server, &registry).await });
    assert!(served.await.unwrap().is_err(), "bad ticket must reject");
    // 无任何 MCP 响应帧：读侧立即 EOF
    assert!(read_frame(&mut client).await.is_err());
}

#[tokio::test]
async fn inbound_peer_mismatch_rejects_stream() {
    let now = crate::audit::now_unix_ms() / 1000;
    let peers = InboundPeers::default();
    peers.record(other_peer()); // 门禁记录的对端不是 ticket 的 bridge_peer
    let registry = registry_for(new_endpoint(
        peers,
        TicketLedger::default(),
        AuditSink::default(),
    ));
    let (hub, mut inbound) = LoopbackHub::new(8, 1 << 20);
    let ticket = fresh_ticket("t-mismatch", now);
    let mut client = open_bridge_stream(&hub, &ticket).await;
    let server = inbound.recv().await.unwrap();
    let served = tokio::spawn(async move { dispatch_inbound(server, &registry).await });
    assert!(served.await.unwrap().is_err(), "peer mismatch must reject");
    assert!(read_frame(&mut client).await.is_err());
}

#[tokio::test]
async fn no_observed_inbound_peer_rejects_stream() {
    let now = crate::audit::now_unix_ms() / 1000;
    let peers = InboundPeers::default(); // 门禁未记录任何对端
    let registry = registry_for(new_endpoint(
        peers,
        TicketLedger::default(),
        AuditSink::default(),
    ));
    let (hub, mut inbound) = LoopbackHub::new(8, 1 << 20);
    let ticket = fresh_ticket("t-nopeer", now);
    let mut client = open_bridge_stream(&hub, &ticket).await;
    let server = inbound.recv().await.unwrap();
    let served = tokio::spawn(async move { dispatch_inbound(server, &registry).await });
    assert!(served.await.unwrap().is_err(), "unknown peer must reject");
    assert!(read_frame(&mut client).await.is_err());
}

#[tokio::test]
async fn same_ticket_second_stream_rejected() {
    let now = crate::audit::now_unix_ms() / 1000;
    let ledger = TicketLedger::default();
    let peers = InboundPeers::default();
    peers.record(bridge_peer());
    let registry = Arc::new(registry_for(new_endpoint(
        peers,
        ledger,
        AuditSink::default(),
    )));
    let (hub, mut inbound) = LoopbackHub::new(8, 1 << 20);
    let ticket = fresh_ticket("t-once", now);

    // 第一次受理：开流 + 发票据，随即断线，受理正常结束
    let client1 = open_bridge_stream(&hub, &ticket).await;
    let server1 = inbound.recv().await.unwrap();
    let reg1 = registry.clone();
    let served1 = tokio::spawn(async move { dispatch_inbound(server1, &reg1).await });
    drop(client1);
    served1.await.unwrap().unwrap();

    // 第二次同票据：一次性查重拒绝（AlreadyUsed），带原因关闭
    let mut client2 = open_bridge_stream(&hub, &ticket).await;
    let server2 = inbound.recv().await.unwrap();
    let reg2 = registry.clone();
    let served2 = tokio::spawn(async move { dispatch_inbound(server2, &reg2).await });
    let err = served2.await.unwrap().unwrap_err();
    assert!(err.to_string().contains("already used"), "{err}");
    assert!(read_frame(&mut client2).await.is_err());
}
