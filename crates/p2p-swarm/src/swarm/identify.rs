//! 内置 identify v1（INTEROP IV2）：design §5.4 控制协议 /p2p-base/identify/1
//! 的 swarm 应答侧实现。定位 = 交换公钥、监听地址、观测地址（specs/identify.md）。
//!
//! 语义边界：Request→Response 一问一答，一流一事务，风格对齐 ping；身份认定
//! 只来自握手，响应 pubkey 仅供发起端交叉核对，不一致按协议违规断流；响应
//! 不落盘，可缓存（TTL ≤60s，由调用方决定）；地址观测接线（把 learned 地址
//! 喂回 set_observed_addrs）属后续卡，本卡只做协议本体。协议 ID 复用 relay
//! crate 已登记常量（单一事实源）；应答侧不抢占用户 handler：registry 已含
//! 该 ID 时跳过注入。发起端客户端见子模块 [client]。

mod client;

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::identify::{AddrMsg, Request, Response, PROTOCOL_VERSION};
use p2p_protocol::{read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId};
use p2p_transport::TransportAddr;
use prost::Message;

pub use client::IdentifyInfo;

/// design §5.4 内置 identify 协议 ID；与 p2p_relay::proto_ids::IDENTIFY 同源。
pub const IDENTIFY_PROTOCOL: &str = p2p_relay::proto_ids::IDENTIFY;

/// 单事务超时（覆盖开流、写请求、读响应全程）。
const IDENTIFY_TIMEOUT: Duration = Duration::from_secs(10);

/// 应答 software 字段；形如 "p2p-base/0.1.0"。
pub const SOFTWARE: &str = concat!("p2p-base/", env!("CARGO_PKG_VERSION"));

fn identify_id() -> ProtocolId {
    ProtocolId::new(IDENTIFY_PROTOCOL).expect("valid identify protocol id")
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// 连接远端观测表：peer → 当前连接的远端端点（本端 socket 视角）。
/// accept/直连拨号入池时刷新，identify 应答的观测地址数据源。
/// 中继电路不记录（mux 观测不到对端真实地址，应答观测字段缺省）。
/// 表随 peer 数有界增长（每 peer 一条，新连接覆盖旧值）。
pub(crate) struct RemoteAddrs(Mutex<HashMap<PeerId, p2p_mux::RemoteEndpoint>>);

impl RemoteAddrs {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// endpoint=None（mux 观测不到）时跳过：缺观测不是错误，应答侧缺省即可。
    pub(crate) fn note(&self, peer: PeerId, endpoint: Option<p2p_mux::RemoteEndpoint>) {
        let Some(endpoint) = endpoint else { return };
        self.0
            .lock()
            .expect("remote addrs lock")
            .insert(peer, endpoint);
    }

    fn get(&self, peer: &PeerId) -> Option<p2p_mux::RemoteEndpoint> {
        self.0.lock().expect("remote addrs lock").get(peer).copied()
    }
}

fn addr_msg_of(quic: bool, addr: SocketAddr) -> AddrMsg {
    AddrMsg {
        quic,
        ip: addr.ip().to_string(),
        port: u32::from(addr.port()),
    }
}

fn addr_msg(addr: &TransportAddr) -> AddrMsg {
    match addr {
        TransportAddr::Quic { ip, port } => AddrMsg {
            quic: true,
            ip: ip.to_string(),
            port: u32::from(*port),
        },
        TransportAddr::Tcp { ip, port } => AddrMsg {
            quic: false,
            ip: ip.to_string(),
            port: u32::from(*port),
        },
    }
}

/// 应答侧：读请求 → 版本校验 → 回响应，返回即关流。帧上限由 read_frame 统一把关。
pub struct IdentifyHandler {
    id: ProtocolId,
    pubkey: [u8; 32],
    listen_addrs: Vec<AddrMsg>,
    software: String,
    remote: Arc<RemoteAddrs>,
}

impl IdentifyHandler {
    pub(crate) fn new(
        keypair: &Keypair,
        listen_addrs: &[TransportAddr],
        remote: Arc<RemoteAddrs>,
    ) -> Self {
        Self {
            id: identify_id(),
            pubkey: keypair.public(),
            listen_addrs: listen_addrs.iter().map(addr_msg).collect(),
            software: SOFTWARE.to_string(),
            remote,
        }
    }

    fn respond(&self, peer: Option<PeerId>) -> Response {
        let observed_addr = peer
            .and_then(|p| self.remote.get(&p))
            .map(|e| addr_msg_of(e.quic, e.addr));
        Response {
            pubkey: self.pubkey.to_vec(),
            listen_addrs: self.listen_addrs.clone(),
            observed_addr,
            software: Some(self.software.clone()),
        }
    }

    async fn reply(&self, peer: Option<PeerId>, mut stream: BoxedStream) -> io::Result<()> {
        let req = read_frame(&mut stream).await?;
        let req = Request::decode(req.as_slice())
            .map_err(|e| invalid(format!("identify request decode: {e}")))?;
        if req.protocol_version != PROTOCOL_VERSION {
            // 版本不匹配 = 显式拒绝：留告警后关流，不猜测降级（design §5.2）
            tracing::warn!(
                peer = ?peer,
                version = req.protocol_version,
                "identify protocol_version mismatch, rejecting"
            );
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "identify protocol_version mismatch",
            ));
        }
        write_frame(&mut stream, &self.respond(peer).encode_to_vec()).await
    }
}

#[async_trait::async_trait]
impl ProtocolHandler for IdentifyHandler {
    fn protocol(&self) -> ProtocolId {
        self.id.clone()
    }

    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        self.reply(Some(peer), stream).await
    }

    /// 裸流（盲拨/回环）无身份上下文：应答照常，观测地址缺省。
    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        self.reply(None, stream).await
    }
}

/// registry 无 identify handler 时注入内置实现；用户 handler 优先（测试可抢注）。
pub(crate) fn registry_with_identify(
    registry: Arc<HandlerRegistry>,
    builtin: IdentifyHandler,
) -> Arc<HandlerRegistry> {
    if registry.get(&identify_id()).is_some() {
        return registry;
    }
    let mut next = HandlerRegistry::default();
    for id in registry.protocols() {
        if let Some(handler) = registry.get(&id) {
            next.register(handler);
        }
    }
    next.register(Arc::new(builtin));
    Arc::new(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler_with(remote: Arc<RemoteAddrs>) -> IdentifyHandler {
        let kp = Keypair::from_seed(&[9u8; 32]);
        IdentifyHandler::new(&kp, &[], remote)
    }

    #[tokio::test]
    async fn happy_path_fills_pubkey_and_observed() {
        let kp = Keypair::from_seed(&[9u8; 32]);
        let peer = kp.peer_id();
        let remote = Arc::new(RemoteAddrs::new());
        remote.note(
            peer,
            Some(p2p_mux::RemoteEndpoint {
                quic: true,
                addr: "127.0.0.1:51000".parse().expect("addr"),
            }),
        );
        let handler = Arc::new(handler_with(remote.clone()));
        let (mut tx, rx) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move { handler.reply(Some(peer), Box::new(rx)).await });
        let req = Request {
            protocol_version: PROTOCOL_VERSION,
        };
        write_frame(&mut tx, &req.encode_to_vec())
            .await
            .expect("write");
        let resp = read_frame(&mut tx).await.expect("read response");
        server.await.expect("reply task").expect("reply ok");
        let resp = Response::decode(resp.as_slice()).expect("decode");
        assert_eq!(resp.pubkey, kp.public().to_vec());
        assert_eq!(resp.listen_addrs.len(), 0);
        let observed = resp.observed_addr.expect("observed filled");
        assert!(observed.quic);
        assert_eq!(observed.ip, "127.0.0.1");
        assert_eq!(observed.port, 51000);
        assert_eq!(resp.software.as_deref(), Some(SOFTWARE));
        assert_eq!(SOFTWARE, "p2p-base/0.1.0");
    }

    #[tokio::test]
    async fn version_mismatch_rejects_without_response() {
        let handler = Arc::new(handler_with(Arc::new(RemoteAddrs::new())));
        let (mut tx, rx) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move { handler.reply(None, Box::new(rx)).await });
        let bad = Request {
            protocol_version: PROTOCOL_VERSION + 1,
        };
        write_frame(&mut tx, &bad.encode_to_vec())
            .await
            .expect("write");
        let err = server.await.expect("reply task").expect_err("must reject");
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
        let left = read_frame(&mut tx).await.expect_err("no response follows");
        assert_eq!(left.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn registry_with_identify_keeps_user_handler_and_injects_when_absent() {
        struct User;
        #[async_trait::async_trait]
        impl ProtocolHandler for User {
            fn protocol(&self) -> ProtocolId {
                identify_id()
            }
            async fn handle(&self, _stream: BoxedStream) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut registry = HandlerRegistry::default();
        let user: Arc<dyn ProtocolHandler> = Arc::new(User);
        registry.register(user.clone());
        let merged = registry_with_identify(
            Arc::new(registry),
            handler_with(Arc::new(RemoteAddrs::new())),
        );
        assert!(Arc::ptr_eq(
            &merged.get(&identify_id()).expect("handler"),
            &user
        ));
        let empty = registry_with_identify(
            Arc::new(HandlerRegistry::default()),
            handler_with(Arc::new(RemoteAddrs::new())),
        );
        assert!(empty.get(&identify_id()).is_some());
    }
}
