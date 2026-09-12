//! W-TC 双节点夹具：真实回环 HTTP 服务 + WS echo 服务 + 访侧 connect 形态
//! 装配（镜像 apps/cli connect.rs 的 NodeTunnelOpener + LocalProxy）。复用
//! tunnel_common::NodeFactory（裸流工厂，禁包 new_stream——协议 ID 恰一帧，
//! specs/tunnel.md §2.1 装配 MUST）。WS 帧/加密助手在 ws 子模块。断言在
//! tunnel_connect_wave.rs。
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use p2p::Node;
use p2p_identity::PeerId;
use p2p_tunnel::{
    LocalProxy, ProxyCtx, TunnelAuditRecord, TunnelClient, TunnelError, TunnelGate, TunnelIo,
    TunnelOpener, TunnelResponder, TunnelServeConfig, TunnelTicket,
};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

mod ws;

pub use ws::{ws_accept, ws_recv_frame, ws_send_frame};

/// HTTP 服务固定 body（断言基准）。
pub const HTTP_BODY: &str = "hello-from-a-http";
/// WS echo 文本帧载荷。
pub const WS_TEXT: &str = "hi-ws-echo";
/// 单步等待上限：本地 loopback 全链毫秒级，15s 为宽松护栏。
const STEP: Duration = Duration::from_secs(15);

/// 回环 HTTP 服务：任意请求返回固定 body（Connection: close 收口）。
pub async fn spawn_http_service() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(serve_http_get(sock));
        }
    });
    port
}

async fn serve_http_get(mut sock: TcpStream) {
    if p2p_tunnel::read_head(&mut sock).await.is_err() {
        return;
    }
    let reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        HTTP_BODY.len(),
        HTTP_BODY
    );
    let _ = sock.write_all(reply.as_bytes()).await;
}

/// 回环 WS echo 服务：RFC6455 升级 + 数据帧原样回显（close 帧回应后收口）。
pub async fn spawn_ws_echo_service() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(echo_ws_session(sock));
        }
    });
    port
}

async fn echo_ws_session(mut sock: TcpStream) {
    let Some(key) = upgrade_key(&mut sock).await else {
        return;
    };
    let reply = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        ws_accept(&key)
    );
    if sock.write_all(reply.as_bytes()).await.is_err() {
        return;
    }
    loop {
        let Ok((opcode, payload)) = ws_recv_frame(&mut sock).await else {
            break;
        };
        match opcode {
            0x8 => {
                let _ = ws_send_frame(&mut sock, 0x8, &payload).await;
                break;
            }
            0x9 => {
                let _ = ws_send_frame(&mut sock, 0xA, &payload).await;
            }
            op => {
                let _ = ws_send_frame(&mut sock, op, &payload).await;
            }
        }
    }
}

/// 读升级请求头并取 Sec-WebSocket-Key（缺头/坏头即 None）。
async fn upgrade_key(sock: &mut TcpStream) -> Option<String> {
    let (raw, _) = p2p_tunnel::read_head(sock).await.ok()?;
    let head = p2p_tunnel::Head::parse(&raw).ok()?;
    head.header("sec-websocket-key").map(str::to_string)
}

/// 访侧 connect 形态 opener（镜像 apps/cli connect.rs NodeTunnelOpener；nonce
/// 以固定 32 hex 承载——会话关联字段，非鉴权凭据，wire.rs §2 口径）。
pub struct ConnectOpener {
    client: TunnelClient<crate::tunnel_common::NodeFactory>,
    peer: PeerId,
}

#[async_trait]
impl TunnelOpener for ConnectOpener {
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError> {
        let ticket = TunnelTicket::new(uid, target, "ab".repeat(16))
            .map_err(|e| TunnelError::Io(std::io::Error::other(e)))?;
        self.client.open(self.peer, &ticket).await
    }
}

/// 双节点真链路装配：A = 被访侧（TcpDialer 真拨回环白名单），B = 访侧。
pub struct ConnectRig {
    pub a: Arc<Node>,
    pub a_peer: PeerId,
    pub b: Arc<Node>,
    pub root: PathBuf,
}

impl Drop for ConnectRig {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 双节点装配：A 挂被访侧 responder（TcpDialer 真拨）+ allowlist 全量开闸；
/// B 直连 A（真链路互联）。
pub async fn rig(tag: &str, allow: Vec<String>) -> ConnectRig {
    let root = std::env::temp_dir().join(format!("tunnel-connect-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a_dir, b_dir) = (root.join("a"), root.join("b"));
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(a_dir)
            .build()
            .await
            .unwrap(),
    );
    let b = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(b_dir)
            .build()
            .await
            .unwrap(),
    );
    let a_peer = a.local_peer_id();
    let gate = TunnelGate::new(TunnelServeConfig {
        allowlist: allow.into_iter().collect(),
        ..Default::default()
    });
    let responder = Arc::new(TunnelResponder::new(
        p2p_tunnel::protocol_id().unwrap(),
        gate.clone(),
        p2p_tunnel::TcpDialer,
    ));
    a.handle_protocol(responder);
    gate.set_enabled(true);
    link(&b, a_peer, &a).await;
    ConnectRig { a, a_peer, b, root }
}

/// 登记对端 TCP 地址并建连（tunnel_common::link 同款真链路互联）。
async fn link(from: &Node, to_peer: PeerId, to: &Node) {
    for addr in to.listen_addrs().into_iter().filter(|a| a.contains("/t")) {
        from.add_peer_address(to_peer, &addr).unwrap();
    }
    from.connect(to_peer).await.unwrap();
}

/// 访侧 connect 形态开启：LocalProxy 绑 127.0.0.1:0 + spawn serve（同
/// connect.rs run；返回句柄交测试驱动本地回环请求）。
pub async fn start_visit_proxy(
    rig: &ConnectRig,
    target_port: u16,
) -> (SocketAddr, Arc<ProxyCtx>, tokio::task::JoinHandle<()>) {
    let opener = Arc::new(ConnectOpener {
        client: TunnelClient::new(crate::tunnel_common::NodeFactory {
            node: rig.b.clone(),
        }),
        peer: rig.a_peer,
    });
    let proxy = LocalProxy::bind(target_port, rig.a_peer.to_string(), opener)
        .await
        .unwrap();
    let addr = proxy.local_addr();
    let ctx = Arc::clone(&proxy.ctx);
    let task = tokio::spawn(proxy.serve());
    (addr, ctx, task)
}

/// 轮询访侧审计快照，返回第 `count` 条已落终态的记录（ended_at != 0 非哨兵）。
pub async fn wait_visit_record(ctx: &ProxyCtx, count: usize) -> TunnelAuditRecord {
    for _ in 0..100 {
        let snapshot = ctx.audit_snapshot().await;
        let mut ended: Vec<_> = snapshot.into_iter().filter(|r| r.ended_at != 0).collect();
        if ended.len() >= count {
            return ended.swap_remove(count - 1);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("visit audit records < {count} within {STEP:?}");
}
