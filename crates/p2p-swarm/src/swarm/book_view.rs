//! 地址簿观测面：只读查询挂到 Swarm，供上层展示与集成测试断言。

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use super::Swarm;

impl Swarm {
    /// 对端当前登记地址（展示串）；未登记返回空。
    pub fn peer_addrs(&self, peer: &PeerId) -> Vec<String> {
        self.address_book.lock().expect("addr lock").addrs_of(peer)
    }

    /// 直连跳用地址：按来源/网段优先级排序，hairpin 候选同级殿后（design §7.3 + E3/E4）。
    /// 返回 (地址, 是否 hairpin 候选)。
    pub(super) fn addresses_of(&self, peer: PeerId) -> Vec<(TransportAddr, bool)> {
        let observed = self.observed_addrs.lock().expect("observed lock").clone();
        self.address_book
            .lock()
            .expect("addr lock")
            .sorted_addrs(&peer, &observed)
    }
}
