//! 打洞宣告地址（design §7.2）：观测地址优先，其后显式宣告或监听地址。
//! 自 mod.rs 搬出（E6），为 swarm/mod.rs 腾出 300 行余量；逻辑不变。

use super::book::filter_loopback;
use super::Swarm;

impl Swarm {
    /// 打洞信令宣告的地址（design §7.2）：观测地址优先（跨网可拨），
    /// 其后为显式宣告或监听地址；去重并过滤 loopback。
    pub(super) fn punch_addrs(&self) -> Vec<p2p_transport::TransportAddr> {
        let mut out: Vec<p2p_transport::TransportAddr> = Vec::new();
        let mut push_all = |addrs: &[p2p_transport::TransportAddr]| {
            for addr in addrs {
                if !out.contains(addr) {
                    out.push(addr.clone());
                }
            }
        };
        push_all(&self.observed_addrs.lock().expect("observed lock"));
        if self.advertised_addrs.is_empty() {
            push_all(&self.listen_addrs);
        } else {
            push_all(&self.advertised_addrs);
        }
        filter_loopback(out)
    }

    pub(super) fn punch_addrs_strs(&self) -> Vec<String> {
        self.punch_addrs().iter().map(ToString::to_string).collect()
    }
}
