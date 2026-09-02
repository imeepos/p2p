//! rendezvous 服务端：按 namespace 维护带 TTL 的注册表，校验签名后应答查询。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use crate::cache::MemCache;
use crate::AddrCache;
use crate::rendezvous::link::{RendezvousConn, RendezvousError};
use crate::rendezvous::messages::{
    request, verify_register, AddrMsg, PeerEntry, Query, Register, Request, Response,
};

/// rendezvous 注册表：namespace → 带 TTL 的地址缓存。
#[derive(Default)]
pub struct RendezvousRegistry {
    namespaces: Mutex<HashMap<String, MemCache>>,
}

impl RendezvousRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理注册：签名校验失败返回 Err（失败路径留日志，禁止静默）。
    pub fn register(&self, reg: &Register) -> Result<(), String> {
        if !verify_register(reg) {
            tracing::warn!(
                target: "p2p_discovery",
                "rendezvous register rejected: bad signature, peer {:?}",
                reg.peer_id
            );
            return Err("bad signature".to_string());
        }
        let peer: [u8; 32] = reg
            .peer_id
            .as_slice()
            .try_into()
            .map_err(|_| "malformed peer_id".to_string())?;
        let addrs: Vec<TransportAddr> = reg.addrs.iter().filter_map(AddrMsg::to_addr).collect();
        let ttl = Duration::from_secs(u64::from(reg.ttl_secs));
        self.namespaces
            .lock()
            .expect("registry lock")
            .entry(reg.namespace.clone())
            .or_default()
            .put(PeerId::from_bytes(peer), addrs, ttl);
        Ok(())
    }

    /// 应答查询：peer_id 为空返回整个 namespace 的未过期条目。
    pub fn query(&self, q: &Query) -> Response {
        let target: Option<PeerId> = q
            .peer_id
            .as_slice()
            .try_into()
            .ok()
            .map(PeerId::from_bytes);
        let mut map = self.namespaces.lock().expect("registry lock");
        let cache = map.entry(q.namespace.clone()).or_default();
        let peers = cache
            .snapshot()
            .into_iter()
            .filter(|(peer, _)| target.is_none_or(|t| *peer == t))
            .map(|(peer, addrs)| PeerEntry {
                peer_id: peer.as_bytes().to_vec(),
                addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
            })
            .collect();
        Response { error: String::new(), peers }
    }
}

/// 在一条连接上持续服务：Register 校验入库，Query 应答；流关闭即返回。
pub async fn serve_link(
    conn: &mut RendezvousConn,
    registry: &RendezvousRegistry,
) -> Result<(), RendezvousError> {
    loop {
        let req = conn.recv_msg::<Request>().await?;
        let resp = match req.kind {
            Some(request::Kind::Register(reg)) => match registry.register(&reg) {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(e),
            },
            Some(request::Kind::Query(q)) => registry.query(&q),
            None => Response::error("missing request kind".to_string()),
        };
        conn.send_msg(&resp).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_identity::Keypair;
    use std::sync::Arc;

    use crate::rendezvous::link::mock::conn_from_duplex;
    use crate::rendezvous::messages::sign_register;

    fn sample_addrs() -> Vec<TransportAddr> {
        vec![TransportAddr::Quic { ip: "10.0.0.5".parse().unwrap(), port: 4000 }]
    }

    #[test]
    fn registry_rejects_tampered_register() {
        let kp = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.addrs[0].port = 9999;
        let registry = RendezvousRegistry::new();
        assert!(registry.register(&reg).is_err());
    }

    #[test]
    fn registry_rejects_wrong_peer_id() {
        let kp = Keypair::generate();
        let other = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.peer_id = other.peer_id().as_bytes().to_vec();
        let registry = RendezvousRegistry::new();
        assert!(registry.register(&reg).is_err());
    }

    #[test]
    fn registry_accepts_valid_register_and_query() {
        let kp = Keypair::generate();
        let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        let registry = RendezvousRegistry::new();
        registry.register(&reg).expect("register ok");
        let resp = registry.query(&Query {
            namespace: "room-a".into(),
            peer_id: reg.peer_id.clone(),
        });
        assert!(resp.error.is_empty());
        assert_eq!(resp.peers.len(), 1);
        assert_eq!(resp.peers[0].peer_id, reg.peer_id);
    }

    #[tokio::test]
    async fn full_duplex_client_server_roundtrip() {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let mut client = conn_from_duplex(client_side);
        let registry = Arc::new(RendezvousRegistry::new());
        let server_registry = registry.clone();
        let server_task = tokio::spawn(async move {
            let mut server = conn_from_duplex(server_side);
            let _ = serve_link(&mut server, &server_registry).await;
        });

        let kp = Keypair::generate();
        let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        let resp = client
            .roundtrip(Request::register(reg.clone()))
            .await
            .expect("register roundtrip");
        resp.ensure_ok().expect("register accepted");

        let resp = client
            .roundtrip(Request::query("room-a".into(), reg.peer_id.clone()))
            .await
            .expect("query roundtrip");
        assert!(resp.error.is_empty());
        assert_eq!(resp.peers.len(), 1);
        assert_eq!(resp.peers[0].peer_id, reg.peer_id);
        server_task.abort();
    }

    #[tokio::test]
    async fn bad_signature_gets_error_response() {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let mut client = conn_from_duplex(client_side);
        let registry = Arc::new(RendezvousRegistry::new());
        let server_registry = registry.clone();
        let server_task = tokio::spawn(async move {
            let mut server = conn_from_duplex(server_side);
            let _ = serve_link(&mut server, &server_registry).await;
        });

        let kp = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.addrs[0].port = 9999; // 篡改后签名不匹配
        let resp = client
            .roundtrip(Request::register(reg))
            .await
            .expect("roundtrip");
        assert!(resp.ensure_ok().is_err());
        server_task.abort();
    }

    #[tokio::test]
    async fn query_unknown_peer_returns_empty() {
        let registry = RendezvousRegistry::new();
        let kp = Keypair::generate();
        let unknown = kp.peer_id().as_bytes().to_vec();
        let resp = registry.query(&Query {
            namespace: "room-a".into(),
            peer_id: unknown,
        });
        assert!(resp.error.is_empty());
        assert!(resp.peers.is_empty());
    }
}
