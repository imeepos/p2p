//! 节点装配开关解析（service-registry-design §2 双读 + §4.2 fail-safe）：
//! 把 services.json 注册表与既有配置字段解析成六个显式布尔，供节点装配层
//! （NodeConfig / GUI state / CLI daemon）消费。本模块不接业务命令面——
//! 装配处必须传与 authz 相同的 data_dir（§4.4 同源同根）。

use std::path::Path;

use crate::registry::{ServiceId, ServiceRegistry};
use crate::store::load_registry;

/// B1 装配消费的六服务生效值（§2 闭集子集，resolve 后显式布尔）。
///
/// 收编型（mdns/lan_only）= 文件条目优先 → legacy 入参 → 默认；显式化型 =
/// 文件条目优先 → 默认 on。显式化型只是「开关 AND 配置」的开关位，配置侧
/// （bootstrap/relay/observation 列表非空、public_only 策略）仍由装配输入决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeServiceSwitches {
    /// discovery.mdns 生效值。
    pub mdns: bool,
    /// net.lan_only 生效值。
    pub lan_only: bool,
    /// net.rendezvous_register 开关位。
    pub rendezvous_register: bool,
    /// net.relay 开关位。
    pub relay: bool,
    /// net.observe 开关位。
    pub observe: bool,
    /// serve.rendezvous_server 开关位。
    pub rendezvous_server: bool,
    /// serve.ftp 开关位（Boolean 默认关；FTP 服务端装配另需 ftp.json 配置）。
    pub ftp: bool,
}

impl NodeServiceSwitches {
    /// 纯解析（§2 双读规则）：registry 文件条目权威，收编型缺失回落 legacy。
    pub fn resolve(registry: &ServiceRegistry, legacy_mdns: bool, legacy_lan_only: bool) -> Self {
        let value = |id: ServiceId| registry.file_value(id);
        Self {
            mdns: ServiceId::DISCOVERY_MDNS
                .resolve(value(ServiceId::DISCOVERY_MDNS), Some(legacy_mdns)),
            lan_only: ServiceId::NET_LAN_ONLY
                .resolve(value(ServiceId::NET_LAN_ONLY), Some(legacy_lan_only)),
            rendezvous_register: ServiceId::NET_RENDEZVOUS_REGISTER
                .resolve(value(ServiceId::NET_RENDEZVOUS_REGISTER), None),
            relay: ServiceId::NET_RELAY.resolve(value(ServiceId::NET_RELAY), None),
            observe: ServiceId::NET_OBSERVE.resolve(value(ServiceId::NET_OBSERVE), None),
            rendezvous_server: ServiceId::SERVE_RENDEZVOUS_SERVER
                .resolve(value(ServiceId::SERVE_RENDEZVOUS_SERVER), None),
            ftp: ServiceId::SERVE_FTP.resolve(value(ServiceId::SERVE_FTP), None),
        }
    }

    /// 落盘读取（§4.2 节点装配 fail-safe）：读失败（损坏/版本不符/越闭集）
    /// 显式 warn 后按空表解析——布尔型=关、显式化型=既有配置语义、收编型=
    /// legacy 字段。显式报错面属服务开关管理命令（B3），装配不因开关文件
    /// 损坏拒启动，但告警必须留痕。
    pub fn load(data_dir: &Path, legacy_mdns: bool, legacy_lan_only: bool) -> Self {
        match load_registry(data_dir) {
            Ok(registry) => Self::resolve(&registry, legacy_mdns, legacy_lan_only),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    data_dir = %data_dir.display(),
                    "services.json 读取失败，节点装配按默认值 fail-safe"
                );
                Self::resolve(&ServiceRegistry::default(), legacy_mdns, legacy_lan_only)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 隔离临时目录（进程内自增计数防并发互踩，同 store 测试约定）。
    fn scratch_dir(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "p2p-service-switches-{}-{}-{}",
            tag,
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 双读两分支锚点（§2/§8.3）：文件条目权威、无条目回落 legacy。
    #[test]
    fn resolve_dual_read_both_branches() {
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::DISCOVERY_MDNS, false, 1);
        reg.set_enabled(ServiceId::NET_LAN_ONLY, true, 2);
        let resolved = NodeServiceSwitches::resolve(&reg, true, false);
        assert!(!resolved.mdns, "文件条目 false 权威，压过 legacy true");
        assert!(resolved.lan_only, "文件条目 true 权威，压过 legacy false");

        let resolved = NodeServiceSwitches::resolve(&ServiceRegistry::default(), true, false);
        assert!(resolved.mdns, "无条目回落 legacy enableMdns");
        assert!(!resolved.lan_only, "无条目回落 legacy lanOnly");
    }

    #[test]
    fn resolve_explicit_kinds_default_on_and_file_entry_wins() {
        let resolved = NodeServiceSwitches::resolve(&ServiceRegistry::default(), false, false);
        assert!(resolved.rendezvous_register && resolved.relay && resolved.observe);
        assert!(resolved.rendezvous_server, "显式化型默认 on 不改行为");
        assert!(!resolved.ftp, "serve.ftp 布尔型默认关（零行为变化）");

        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::NET_RELAY, false, 1);
        reg.set_enabled(ServiceId::SERVE_RENDEZVOUS_SERVER, false, 2);
        let resolved = NodeServiceSwitches::resolve(&reg, false, false);
        assert!(!resolved.relay && !resolved.rendezvous_server, "条目权威");
        assert!(
            resolved.rendezvous_register && resolved.observe,
            "未设条目仍默认 on"
        );
    }

    #[test]
    fn load_missing_file_mirrors_legacy_fields() {
        let dir = scratch_dir("missing");
        let resolved = NodeServiceSwitches::load(&dir, true, false);
        assert!(resolved.mdns);
        assert!(!resolved.lan_only);
        assert!(resolved.relay, "显式化型不受 legacy 入参影响");
    }

    #[test]
    fn load_corrupt_file_fails_safe_to_defaults_with_legacy() {
        let dir = scratch_dir("corrupt");
        std::fs::write(crate::store::services_path(&dir), "{ not json").unwrap();
        let resolved = NodeServiceSwitches::load(&dir, true, false);
        assert_eq!(
            resolved,
            NodeServiceSwitches::resolve(&ServiceRegistry::default(), true, false),
            "读失败 = 空表 fail-safe，不静默也拒不启动"
        );
        assert!(resolved.mdns && !resolved.lan_only);
    }

    #[test]
    fn load_persisted_entries_override_legacy_fields() {
        let dir = scratch_dir("persisted");
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::DISCOVERY_MDNS, false, 1);
        reg.set_enabled(ServiceId::NET_RELAY, false, 2);
        crate::store::save_registry(&dir, &reg).unwrap();
        let resolved = NodeServiceSwitches::load(&dir, true, false);
        assert!(!resolved.mdns, "落盘条目压过 legacy enableMdns");
        assert!(!resolved.relay, "落盘条目压过显式化型默认 on");
    }
}
