//! 服务闭集（service-registry-design §2）：首批 10 项，只加不删（废用标
//! deprecated）。id/型别/默认值单一真值源 = 本表（描述不进 services.json，
//! plan §0.2 防双真值源漂移）；数据一致性测试逐字锚定设计表，改表即测试红
//! （沿 p2p-authz permissions.rs 先例）。

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 三型语义（plan §0.3 逐字）：
/// 布尔型=新显式闸安全默认关；显式化型=开关 AND 配置双条件默认 on 不改行为；
/// 收编型=既有开关收编进注册表，迁移期双读。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    /// 布尔型（新显式闸，安全默认关）
    Boolean,
    /// 显式化型（开关 AND 配置双条件，默认 on 不改行为）
    Explicit,
    /// 收编型（既有开关收编，迁移期双读：条目优先，缺失回落原配置字段）
    Adopted,
}

impl ServiceKind {
    /// 序列化词形（gui-contract §20.3 ServiceKindJson 逐字）。
    pub const fn as_str(self) -> &'static str {
        match self {
            ServiceKind::Boolean => "boolean",
            ServiceKind::Explicit => "explicit",
            ServiceKind::Adopted => "adopted",
        }
    }
}

/// 闭集 service_id。构造仅限本表常量与 [ServiceId::parse]（表外字符串进不来）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(&'static str);

impl ServiceId {
    pub const SERVE_LLM_SHARE: ServiceId = ServiceId("serve.llm_share");
    pub const SERVE_TUNNEL: ServiceId = ServiceId("serve.tunnel");
    pub const SERVE_A2A: ServiceId = ServiceId("serve.a2a");
    pub const SERVE_ACP: ServiceId = ServiceId("serve.acp");
    pub const NET_RENDEZVOUS_REGISTER: ServiceId = ServiceId("net.rendezvous_register");
    pub const NET_RELAY: ServiceId = ServiceId("net.relay");
    pub const NET_OBSERVE: ServiceId = ServiceId("net.observe");
    pub const SERVE_RENDEZVOUS_SERVER: ServiceId = ServiceId("serve.rendezvous_server");
    pub const DISCOVERY_MDNS: ServiceId = ServiceId("discovery.mdns");
    pub const NET_LAN_ONLY: ServiceId = ServiceId("net.lan_only");

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// 闭集精确匹配解析：表外 id 一律 None（命令面报错与 serde 拒读的唯一入口）。
    pub fn parse(key: &str) -> Option<Self> {
        REGISTRY.iter().copied().find(|s| s.0 == key)
    }

    pub fn registry() -> &'static [Self] {
        REGISTRY
    }

    /// 型别与默认值（§2 表逐字；改表即测试红）。
    pub fn kind(self) -> ServiceKind {
        match self {
            ServiceId::SERVE_LLM_SHARE | ServiceId::SERVE_TUNNEL => ServiceKind::Boolean,
            ServiceId::DISCOVERY_MDNS | ServiceId::NET_LAN_ONLY => ServiceKind::Adopted,
            _ => ServiceKind::Explicit,
        }
    }

    /// 默认 enabled（§2 表「默认值」列逐字；收编型为「无条目且无 legacy 字段」
    /// 时的兜底，等于现状出厂值）。
    pub fn default_enabled(self) -> bool {
        !matches!(
            self,
            ServiceId::SERVE_LLM_SHARE | ServiceId::SERVE_TUNNEL | ServiceId::NET_LAN_ONLY
        )
    }

    /// 生效值解析（§2 双读规则）：收编型 = 文件条目优先 → legacy 字段 → 默认；
    /// 其余型 = 文件条目优先 → 默认。legacy_fallback 只对收编型有意义，装配处
    /// 从 GuiConfig 取（discovery.mdns←enableMdns、net.lan_only←lanOnly）。
    pub fn resolve(self, file_value: Option<bool>, legacy_fallback: Option<bool>) -> bool {
        match self.kind() {
            ServiceKind::Adopted => file_value
                .or(legacy_fallback)
                .unwrap_or(self.default_enabled()),
            ServiceKind::Boolean | ServiceKind::Explicit => {
                file_value.unwrap_or(self.default_enabled())
            }
        }
    }
}

/// §2 首批闭集（10 项），顺序与设计表一致。
static REGISTRY: &[ServiceId] = &[
    ServiceId::SERVE_LLM_SHARE,
    ServiceId::SERVE_TUNNEL,
    ServiceId::SERVE_A2A,
    ServiceId::SERVE_ACP,
    ServiceId::NET_RENDEZVOUS_REGISTER,
    ServiceId::NET_RELAY,
    ServiceId::NET_OBSERVE,
    ServiceId::SERVE_RENDEZVOUS_SERVER,
    ServiceId::DISCOVERY_MDNS,
    ServiceId::NET_LAN_ONLY,
];

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for ServiceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for ServiceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        ServiceId::parse(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("service_id 不在闭集登记表: {key}")))
    }
}

/// services.json 条目（最小三字段，plan §0.2）：只含用户显式设置过的服务。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub service_id: ServiceId,
    pub enabled: bool,
    pub updated_at: u64,
}

/// 内存注册表视图 = services.json 条目集（文件真值；未设置的服务不在列，
/// 由 [ServiceId::resolve] 按默认/双读推导）。同 id 重复条目按后写者胜归一。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceRegistry {
    entries: Vec<ServiceEntry>,
}

impl ServiceRegistry {
    /// 由条目集构造；同 service_id 重复按后写者胜归一（确定性，防文件脏数据外溢）。
    pub fn new(mut entries: Vec<ServiceEntry>) -> Self {
        entries.sort_by_key(|e| e.updated_at);
        let mut normalized: Vec<ServiceEntry> = Vec::new();
        for entry in entries {
            match normalized
                .iter_mut()
                .find(|e| e.service_id == entry.service_id)
            {
                Some(slot) => *slot = entry,
                None => normalized.push(entry),
            }
        }
        Self {
            entries: normalized,
        }
    }

    pub fn entries(&self) -> &[ServiceEntry] {
        &self.entries
    }

    /// 文件真值：条目存在返回 Some(enabled)，未设置返回 None（非默认值）。
    pub fn file_value(&self, id: ServiceId) -> Option<bool> {
        self.entries
            .iter()
            .find(|e| e.service_id == id)
            .map(|e| e.enabled)
    }

    /// upsert 开关（写路径先重读磁盘合并后调用本方法再落盘，§4.5）。
    /// `now_unix` 由调用方注入（时钟不进存储层，沿 authz Clock 先例）。
    pub fn set_enabled(&mut self, id: ServiceId, enabled: bool, now_unix: u64) {
        match self.entries.iter_mut().find(|e| e.service_id == id) {
            Some(entry) => {
                entry.enabled = enabled;
                entry.updated_at = now_unix;
            }
            None => self.entries.push(ServiceEntry {
                service_id: id,
                enabled,
                updated_at: now_unix,
            }),
        }
    }
}

/// 生效值推导：`legacy_fallback` 仅收编型消费（见 [ServiceId::resolve]）。
pub fn effective_enabled(
    id: ServiceId,
    registry: &ServiceRegistry,
    legacy_fallback: Option<bool>,
) -> bool {
    id.resolve(registry.file_value(id), legacy_fallback)
}
