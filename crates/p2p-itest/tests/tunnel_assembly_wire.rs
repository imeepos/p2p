//! W-T4 装配线格式严格 itest（no-tolerance）：流上协议 ID 恰一帧（由 inbound
//! 分发消费），handler 收到的首业务帧必须即票据 JSON。双写装配（工厂包
//! `Node::new_stream`）在此严格探针下必红——responder 的 read_ticket 容忍
//! 循环是防御层，本文件不依赖它。三个探针：
//! - 干净工厂（open_raw_stream）绿探针：严格线格式 + 回环全绿；
//! - 双写工厂红探针：严格拒绝 BadTicket，永久固化红方向；
//! - 真实夹具工厂（tunnel_common::NodeFactory）红绿枢轴：清扫前红、清扫后绿。
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use p2p::{BoxedStream, Node};
use p2p_identity::PeerId;
use p2p_protocol::{read_frame, write_frame, ProtocolHandler, ProtocolId, StreamFactory};
use p2p_tunnel::{
    protocol_id, PROTOCOL_ID, TunnelClient, TunnelError, TunnelErrorCode, TunnelReply,
    TunnelTicket,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

#[path = "tunnel_common/mod.rs"]
mod tunnel_common;

const TARGET: &str = "127.0.0.1:8014";
const UID: &str = "445566778899aabb";

/// 严格探针 handler：首业务帧必须是票据 JSON（不容忍协议 ID 帧）；
/// 合法则 ack + 单帧回环，越轨则回 BadTicket error 帧。首帧原文留证。
struct StrictProbe {
    protocol: ProtocolId,
    first_frames: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[async_trait]
impl ProtocolHandler for StrictProbe {
    fn protocol(&self) -> ProtocolId {
        self.protocol.clone()
    }

    async fn handle_inbound(&self, _peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let first = read_frame(&mut stream).await?;
        self.first_frames.lock().await.push(first.clone());
        let ticket = match TunnelTicket::decode(&first) {
            Ok(t) => t,
            Err(e) => {
                return reject(&mut stream, format!("first business frame not ticket JSON: {e}"))
                    .await;
            }
        };
        let ack = TunnelReply::ack(&ticket.uid)
            .encode()
            .map_err(io::Error::other)?;
        write_frame(&mut stream, &ack).await?;
        // 单帧回环后收口：证明 ack 后数据面可用即可，不做全量泵。
        let echo = read_frame(&mut stream).await?;
        write_frame(&mut stream, &echo).await
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "strict probe requires authenticated peer",
        ))
    }
}

async fn reject(stream: &mut BoxedStream, message: String) -> io::Result<()> {
    let frame = TunnelReply::error(TunnelErrorCode::BadTicket, message)
        .encode()
        .map_err(io::Error::other)?;
    write_frame(stream, &frame).await
}

/// 被探针工厂：clean=裸流（open_raw_stream，调用方握手），dirty=双写复刻
/// （new_stream 已写协议 ID，TunnelClient 再写一帧）。
struct ProbeFactory {
    node: Arc<Node>,
    dirty: bool,
}

#[async_trait]
impl StreamFactory for ProbeFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream> {
        let opened = if self.dirty {
            self.node.new_stream(*peer, protocol.clone()).await
        } else {
            self.node.open_raw_stream(*peer, protocol.clone()).await
        };
        opened.map_err(|e| io::Error::other(e.to_string()))
    }
}

/// 双节点严格装配：A 挂 StrictProbe，B 已建连、交调用方自选工厂。
struct WireRig {
    a: Arc<Node>,
    b: Arc<Node>,
    a_peer: PeerId,
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
    root: PathBuf,
}

impl Drop for WireRig {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn rig(tag: &str) -> WireRig {
    let root = std::env::temp_dir().join(format!("tunnel-assembly-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let node = |name: &str| {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let a = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(node("a"))
            .build()
            .await
            .unwrap(),
    );
    let b = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(node("b"))
            .build()
            .await
            .unwrap(),
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    a.handle_protocol(Arc::new(StrictProbe {
        protocol: protocol_id().unwrap(),
        first_frames: seen.clone(),
    }));
    for addr in a.listen_addrs().into_iter().filter(|a| a.contains("/t")) {
        b.add_peer_address(a.local_peer_id(), &addr).unwrap();
    }
    b.connect(a.local_peer_id()).await.unwrap();
    let a_peer = a.local_peer_id();
    WireRig {
        a,
        b,
        a_peer,
        seen,
        root,
    }
}

fn fixture_ticket() -> TunnelTicket {
    TunnelTicket::new(UID, TARGET, "cd".repeat(16)).unwrap()
}

/// 绿探针：裸流装配 + 自握手（open_raw_stream），严格线格式全绿且回环可用。
#[tokio::test]
async fn single_write_assembly_passes_strict_wire() {
    let rig = rig("clean").await;
    let client = TunnelClient::new(ProbeFactory {
        node: rig.b.clone(),
        dirty: false,
    });
    let mut io = client
        .open(rig.a_peer, &fixture_ticket())
        .await
        .expect("clean assembly must open strictly");
    assert_wire_and_echo(&mut io).await;
    let seen = rig.seen.lock().await;
    let first = seen.first().expect("恰好一条流被服务");
    assert_not_protocol_frame(first);
    assert_eq!(
        TunnelTicket::decode(first).expect("首业务帧必须是票据 JSON").uid,
        UID
    );
}

/// 红探针（永久）：工厂包 new_stream 的双写装配在严格探针下必被拒绝，
/// handler 首业务帧实测为协议 ID 原文。
#[tokio::test]
async fn double_write_assembly_is_strictly_rejected() {
    let rig = rig("dirty").await;
    let client = TunnelClient::new(ProbeFactory {
        node: rig.b.clone(),
        dirty: true,
    });
    let err = match client.open(rig.a_peer, &fixture_ticket()).await {
        Err(e) => e,
        Ok(_) => panic!("double-write assembly must fail strict wire"),
    };
    let TunnelError::Rejected { code, message } = err else {
        panic!("expected structured rejection, got {err:?}");
    };
    assert_eq!(code, TunnelErrorCode::BadTicket, "{message}");
    let seen = rig.seen.lock().await;
    assert_eq!(
        seen.first().map(Vec::as_slice),
        Some(PROTOCOL_ID.as_bytes()),
        "双写装配下 handler 首业务帧实测为协议 ID 第二帧"
    );
}

/// 红绿枢轴：真实夹具工厂（tunnel_common::NodeFactory）必须过严格线格式。
/// 清扫前该工厂包 new_stream 必红；清扫后（open_raw_stream）绿，防回退。
#[tokio::test]
async fn real_fixture_factory_passes_strict_wire() {
    let rig = rig("fixture").await;
    let client = TunnelClient::new(tunnel_common::NodeFactory {
        node: rig.b.clone(),
    });
    let mut io = client
        .open(rig.a_peer, &fixture_ticket())
        .await
        .expect("真实夹具工厂必须过严格线格式（清扫未完成或回退即红）");
    assert_wire_and_echo(&mut io).await;
    let seen = rig.seen.lock().await;
    let first = seen.last().expect("fixture 流必被服务");
    assert_not_protocol_frame(first);
    TunnelTicket::decode(first).expect("首业务帧必须是票据 JSON");
}

async fn assert_wire_and_echo(io: &mut p2p_tunnel::TunnelIo) {
    io.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    io.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");
}

fn assert_not_protocol_frame(first: &[u8]) {
    assert!(
        !first.starts_with(b"/"),
        "首业务帧不得是协议 ID（双写装配必触此红线）：{}",
        String::from_utf8_lossy(first)
    );
}
