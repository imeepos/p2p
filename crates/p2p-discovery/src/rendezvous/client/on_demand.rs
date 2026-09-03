//! 按需精确查询（社交化发现 P1 原语，social-discovery-plan.md §5）：
//! 对已知 PeerId 一次「查号」——独立建连、查询即断，不进周期循环、
//! 不触发全量拉取；查询能力独立于本端注册（E5/libp2p rendezvous 语义）。

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use super::response_to_peers;
use super::routable_only;
use super::RendezvousClient;
use super::RendezvousError;
use super::Request;
use super::Response;

impl RendezvousClient {
    /// 查询单个对端的最新地址：命中返回地址列表（可能已按
    /// strip_unroutable 过滤），对端未知返回空 Vec。
    pub async fn query_peer(&self, peer_id: PeerId) -> Result<Vec<TransportAddr>, RendezvousError> {
        let mut conn = self.config.link.connect().await?;
        let req = Request::query(self.config.namespace.clone(), peer_id.as_bytes().to_vec());
        conn.send_msg(&req).await?;
        let resp = conn.recv_msg::<Response>().await?;
        resp.ensure_ok().map_err(RendezvousError::Protocol)?;
        let mut addrs = response_to_peers(&resp)
            .into_iter()
            .find(|(p, _)| *p == peer_id)
            .map(|(_, a)| a)
            .unwrap_or_default();
        if self.config.strip_unroutable {
            addrs = routable_only(&addrs).unwrap_or_default();
        }
        Ok(addrs)
    }
}
