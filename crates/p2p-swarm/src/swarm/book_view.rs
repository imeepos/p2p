//! 地址簿观测面：只读查询挂到 Swarm，供上层展示与集成测试断言。

use p2p_identity::PeerId;

use super::Swarm;

impl Swarm {
    /// 对端当前登记地址（展示串）；未登记返回空。
    pub fn peer_addrs(&self, peer: &PeerId) -> Vec<String> {
        self.address_book.lock().expect("addr lock").addrs_of(peer)
    }
}
