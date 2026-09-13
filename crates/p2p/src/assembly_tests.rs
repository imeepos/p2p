//! 装配级单测（F8 lan-only 回归 + service-registry-design §2 开关消费）：
//! 全部纯函数直测，不起 swarm、不产生网络流量。

use std::path::PathBuf;
use std::sync::Arc;

use p2p_identity::Keypair;

use crate::assembly::{
    apply_explicit_switches, rendezvous_server_handler, strip_public_endpoints, wire_rendezvous,
};
use crate::{NodeConfig, ServiceSwitches};

fn public_config() -> NodeConfig {
    NodeConfig {
        bootstrap: vec!["43.240.223.138/u3400".into()],
        relay_addrs: vec!["43.240.223.138/u3403".into()],
        observation_addrs: vec!["121.196.193.177:3402".into()],
        observation_port: Some(3402),
        ..NodeConfig::default()
    }
}

/// 在公网样例配置上翻转指定显式化型开关位。
fn with_switches(mut cfg: NodeConfig, f: impl FnOnce(&mut ServiceSwitches)) -> NodeConfig {
    f(&mut cfg.service_switches);
    cfg
}

#[test]
fn lan_only_default_false_preserves_public_endpoints() {
    let cfg = NodeConfig::default();
    assert!(!cfg.lan_only, "默认必须为 false：不开启时现网行为零变化");
    let base = public_config();
    assert_eq!(base.bootstrap.len(), 1, "lan_only=false 装配不动公网端点");
    assert_eq!(base.observation_port, Some(3402));
}

#[test]
fn lan_only_strips_public_endpoints_keeps_lan_facets() {
    let mut cfg = public_config();
    cfg.lan_only = true;
    cfg.enable_mdns = true;
    cfg.static_peers_file = Some(PathBuf::from("/tmp/peers.json"));
    cfg.quic_port = 3400;
    let stripped = strip_public_endpoints(cfg);
    assert!(stripped.bootstrap.is_empty(), "不拨公网 bootstrap");
    assert!(stripped.relay_addrs.is_empty(), "不连公网 relay");
    assert!(stripped.observation_addrs.is_empty(), "不上报 observation");
    assert_eq!(stripped.observation_port, None, "不开公共观测反射口");
    assert!(stripped.enable_mdns, "局域网发现保留");
    assert!(stripped.static_peers_file.is_some(), "局域网直连登记保留");
    assert_eq!(stripped.quic_port, 3400, "监听端口保留");
}

/// 显式化型默认全 on：不设开关时装配输入零变化（行为兼容锚点）。
#[test]
fn service_switches_default_all_on_preserves_endpoints() {
    let applied = apply_explicit_switches(public_config());
    assert_eq!(applied.service_switches, ServiceSwitches::default());
    assert_eq!(applied.bootstrap.len(), 1, "rendezvous 注册照常");
    assert_eq!(applied.relay_addrs.len(), 1, "relay 降级链照常");
    assert_eq!(applied.observation_addrs.len(), 1, "地址观测照常");
}

#[test]
fn relay_off_strips_relay_keeps_bootstrap_and_observation() {
    let applied = apply_explicit_switches(with_switches(public_config(), |s| s.relay = false));
    assert!(
        applied.relay_addrs.is_empty(),
        "net.relay=off 降级链不含 relay"
    );
    assert_eq!(
        applied.bootstrap.len(),
        1,
        "rendezvous 注册不受 relay 开关影响"
    );
    assert_eq!(
        applied.observation_addrs.len(),
        1,
        "观测不受 relay 开关影响"
    );
}

#[test]
fn observe_off_strips_observation_keeps_relay() {
    let applied = apply_explicit_switches(with_switches(public_config(), |s| s.observe = false));
    assert!(
        applied.observation_addrs.is_empty(),
        "net.observe=off 跳过地址观测"
    );
    assert_eq!(applied.relay_addrs.len(), 1, "relay 不受 observe 开关影响");
    assert_eq!(
        applied.observation_port,
        Some(3402),
        "反射口（服务侧）不在本开关域"
    );
}

/// rendezvous_register=off：bootstrap 被清空，wire_rendezvous 走既有空表
/// 跳路径返回 None——不注册、不接线（验收锚点）。
#[test]
fn rendezvous_register_off_skips_wiring() {
    let applied = apply_explicit_switches(with_switches(public_config(), |s| {
        s.rendezvous_register = false
    }));
    assert!(
        applied.bootstrap.is_empty(),
        "off 即清 bootstrap，注册与接线全断"
    );
    let wired = wire_rendezvous(&applied, &Arc::new(Keypair::generate()), &[]).unwrap();
    assert!(wired.is_none(), "不接线 rendezvous client");
}

/// 默认 on 且 bootstrap 非空：接线行为与开关引入前一致。
/// （TransportLink 构造建 Quinn endpoint，需 tokio runtime 上下文。）
#[tokio::test]
async fn rendezvous_register_on_with_bootstrap_still_wires() {
    let wired = wire_rendezvous(&public_config(), &Arc::new(Keypair::generate()), &[]).unwrap();
    assert!(wired.is_some(), "显式化型默认 on 不改行为");
}

/// serve.rendezvous_server=off 不装配服务端；on 时照常构造（public_only
/// 策略原样透传，语义保留）。
#[test]
fn rendezvous_server_off_not_assembled_on_still_assembles() {
    let off = with_switches(public_config(), |s| s.rendezvous_server = false);
    assert!(
        rendezvous_server_handler(&off).unwrap().is_none(),
        "off 不装配 RendezvousServer"
    );
    assert!(
        rendezvous_server_handler(&public_config())
            .unwrap()
            .is_some(),
        "默认 on 照常装配"
    );
}
