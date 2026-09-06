//! 网络外联声明装配（F8）：GuiConfig → [NetworkNotice]，回落规则与守护进程
//! 装配同源（空列表回落出厂默认），防止「声明」与「实连端点」漂移。

use p2p_cli::notice::{NetworkNotice, PublicEndpoint};

use crate::types::{default_bootstrap, default_observation_addrs, default_relay_addrs, GuiConfig};

/// 空列表回落出厂默认（serde 只兜字段缺失，显式 [] 在装配时兜底，GUI 同规则）。
pub fn factory_fallback(list: &[String], factory: fn() -> Vec<String>) -> Vec<String> {
    if list.is_empty() {
        factory()
    } else {
        list.to_vec()
    }
}

/// 生效公网端点集合：lan-only 时为空（声明不列端点），否则逐类回落出厂默认。
pub fn effective_public_endpoints(cfg: &GuiConfig) -> Vec<PublicEndpoint> {
    if cfg.lan_only {
        return Vec::new();
    }
    vec![
        PublicEndpoint {
            category: "bootstrap",
            purpose: "rendezvous 跨网发现注册与查号",
            addrs: factory_fallback(&cfg.bootstrap, default_bootstrap),
        },
        PublicEndpoint {
            category: "relay",
            purpose: "打洞失败后的中继兜底",
            addrs: factory_fallback(&cfg.relay_addrs, default_relay_addrs),
        },
        PublicEndpoint {
            category: "observation",
            purpose: "学习自身公网映射地址",
            addrs: factory_fallback(&cfg.observation_addrs, default_observation_addrs),
        },
    ]
}

/// start/serve 声明事实源：lan-only 单行声明，公网模式逐类列出意图与地址。
pub fn notice_for_config(cfg: &GuiConfig) -> NetworkNotice {
    if cfg.lan_only {
        NetworkNotice::lan_only()
    } else {
        NetworkNotice::public(effective_public_endpoints(cfg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_only_config_yields_empty_endpoints() {
        let mut cfg = GuiConfig::default();
        cfg.lan_only = true;
        let notice = notice_for_config(&cfg);
        assert!(notice.lan_only);
        assert!(notice.endpoints.is_empty(), "lan-only 不得列公网端点");
        assert!(notice.text().contains("仅局域网"));
        assert!(notice.text().contains("43.240.223.138") == false);
    }

    #[test]
    fn public_config_lists_factory_default_categories() {
        let cfg = GuiConfig::default();
        let notice = notice_for_config(&cfg);
        assert!(!notice.lan_only);
        assert_eq!(notice.endpoints.len(), 3, "出厂默认三类全列（显式可感知）");
        assert!(notice.text().contains("bootstrap"));
        assert!(notice.text().contains("relay"));
        assert!(notice.text().contains("observation"));
    }

    #[test]
    fn explicit_empty_list_falls_back_to_factory_endpoints() {
        let mut cfg = GuiConfig::default();
        cfg.bootstrap = Vec::new();
        let endpoints = effective_public_endpoints(&cfg);
        let bootstrap = endpoints
            .iter()
            .find(|e| e.category == "bootstrap")
            .expect("bootstrap endpoint");
        assert!(
            !bootstrap.addrs.is_empty(),
            "显式 [] 在声明侧同样回落出厂默认，与装配一致"
        );
    }
}
