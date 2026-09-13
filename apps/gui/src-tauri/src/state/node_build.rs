//! GuiConfig → Node 装配（契约 §1 node_start）：服务总控双读（service-
//! registry-design §2/§4.4，services.json 根 = app 数据目录，与 authz 同源
//! 同根）+ 空地址列表回落出厂默认；与 CLI daemon.rs 装配同语义。

use std::path::{Path, PathBuf};

use p2p::{Node, ServiceSwitches};
use p2p_service::NodeServiceSwitches;
use tracing::warn;

use crate::config::{default_bootstrap, default_observation_addrs, default_relay_addrs};
use crate::types::GuiConfig;

/// GuiConfig → Node 装配；`services_root` = services.json 所在数据根（GUI =
/// app 数据目录，非 GuiConfig.dataDir 节点数据目录）。开关生效值经
/// services.json 双读：条目优先，缺失回落 GuiConfig 字段（§2 收编型）。
pub(super) async fn build_node(cfg: &GuiConfig, services_root: &Path) -> Result<Node, String> {
    let switches = NodeServiceSwitches::load(services_root, cfg.enable_mdns, cfg.lan_only);
    let mut builder = Node::builder()
        .quic_port(cfg.quic_port)
        .tcp_port(cfg.tcp_port)
        .bootstrap(with_factory_fallback(&cfg.bootstrap, default_bootstrap))
        .mdns(switches.mdns)
        .lan_only(switches.lan_only)
        .service_switches(ServiceSwitches {
            rendezvous_register: switches.rendezvous_register,
            relay: switches.relay,
            observe: switches.observe,
            rendezvous_server: switches.rendezvous_server,
        })
        .data_dir(PathBuf::from(&cfg.data_dir))
        .relay_addrs(with_factory_fallback(&cfg.relay_addrs, default_relay_addrs))
        .advertised_addrs(cfg.advertised_addrs.clone());
    if let Some(port) = cfg.observation_port {
        builder = builder.observation_responder(port);
    }
    builder = builder.observation_addrs(with_factory_fallback(
        &cfg.observation_addrs,
        default_observation_addrs,
    ));
    builder.build().await.map_err(|e| {
        warn!(error = %e, "节点装配失败");
        format!("节点启动失败: {e}")
    })
}

/// 空列表回落出厂默认：serde 默认只兜字段缺失，落盘的显式 `[]`（旧版本
/// 配置/用户清空）在装配时兜底，兑现空态提示「列表为空时使用出厂默认端点」；
/// 持久层保持原样不回写。
fn with_factory_fallback(list: &[String], factory: fn() -> Vec<String>) -> Vec<String> {
    if list.is_empty() {
        factory()
    } else {
        list.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_addr_lists_fall_back_to_factory_defaults() {
        assert_eq!(
            with_factory_fallback(&[], default_bootstrap),
            default_bootstrap()
        );
        assert_eq!(
            with_factory_fallback(&[], default_relay_addrs),
            default_relay_addrs()
        );
        assert_eq!(
            with_factory_fallback(&[], default_observation_addrs),
            default_observation_addrs()
        );
    }

    #[test]
    fn non_empty_addr_lists_stay_verbatim() {
        let list = vec!["10.0.0.1/u3400".to_string()];
        assert_eq!(with_factory_fallback(&list, default_relay_addrs), list);
    }
}
